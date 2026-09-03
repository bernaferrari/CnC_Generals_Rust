#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

pub(super) const DEFAULT_SKIRMISH_MAP: &str = "Defcon6";
/// C++ `View::m_FOV` is the **horizontal** field of view (View.h:173, View.cpp:53).
/// glam `perspective_rh` takes vertical FOV, so convert at matrix build time.
pub(super) const DEFAULT_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
/// C++ W3DView (W3DView.cpp:549-563): `nearZ = MAP_XY_FACTOR` (MapObject.h:35,
/// 10.0 world units per height-map cell — "Improves zbuffer resolution").
pub(super) const DEFAULT_VIEW_NEAR_CLIP: f32 = 10.0;

/// C++ `CameraClass::Set_View_Plane(hfov, -1)` (WW3D2/camera.cpp:257-261):
/// `height_half = tan(hfov/2) / aspect` ⇒ `vfov = 2*atan(tan(hfov/2)/aspect)`.
#[inline]
pub(super) fn vertical_fov_from_horizontal(hfov_radians: f32, aspect: f32) -> f32 {
    let aspect = aspect.max(0.01);
    2.0 * ((hfov_radians * 0.5).tan() / aspect).atan()
}

#[inline]
pub(super) fn perspective_rh_from_horizontal_fov(
    hfov_radians: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> glam::Mat4 {
    // C++ `CameraClass::Get_D3D_Projection_Matrix` (WW3D2/camera.cpp:707-732)
    // overrides rows 2/3 of `ProjectionTransform` with the D3D depth mapping:
    // NDC z spans [0,1] (near plane → 0, far plane → 1). wgpu expects exactly
    // that window; feeding it a GL-style [-1,1] mapping (glam
    // `perspective_rh`) silently clips everything nearer than the midpoint
    // depth and compresses the rest. x/y columns match glam's RH form so
    // framing is unchanged.
    let vfov = vertical_fov_from_horizontal(hfov_radians, aspect);
    let f = 1.0 / (vfov * 0.5).tan();
    let z22 = far / (near - far);
    let z32 = (near * far) / (near - far);
    glam::Mat4::from_cols(
        glam::Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, z22, -1.0),
        glam::Vec4::new(0.0, 0.0, z32, 0.0),
    )
}

/// C++ W3DView (W3DView.cpp:551-562): `farZ = 1200`, extended ×MAP_XY_FACTOR
/// (12000) whenever the whole terrain can be visible (zoom-out / high pitch).
/// The port keeps the extended value so no zoom level clips the far plane.
pub(super) const DEFAULT_VIEW_FAR_CLIP: f32 = 12_000.0;
pub(super) const DEFAULT_LOADING_PHASE: &str = "Loading assets...";
pub(super) const SHELL_MENU_WINDOW_TITLE: &str = "Command & Conquer Generals Zero Hour";

// Window names from ShellGameLoadScreen.wnd (C++ parity: winCreateFromScript)
pub(super) const LOAD_SCREEN_ROOT: &str = "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen";
pub(super) const LOAD_SCREEN_PROGRESS: &str = "ShellGameLoadScreen.wnd:ProgressLoad";

pub(super) fn pack_ui_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}

pub(super) fn should_keep_logic_running_while_iconic(mode: GameMode) -> bool {
    matches!(
        mode,
        GameMode::Multiplayer | GameMode::Lan | GameMode::Internet
    )
}

pub(super) fn query_window_is_iconic(window: &Window, fallback: bool) -> bool {
    let size = window.inner_size();
    let zero_sized = size.width == 0 || size.height == 0;
    window.is_minimized().unwrap_or(fallback || zero_sized) || zero_sized
}
/// Render-surface extent in logical points (C++ `TheDisplay` width/height).
///
/// The swapchain and every logical-space consumer (WND window manager, UI
/// renderer, tactical viewport) share this size; winit's `inner_size` is
/// physical and must only be used for input-space math.
pub(super) fn render_surface_extent(window: &Window) -> (u32, u32) {
    let scale = window.scale_factor().max(0.0001);
    let size = window.inner_size();
    let w = ((size.width as f64) / scale).round().max(1.0) as u32;
    let h = ((size.height as f64) / scale).round().max(1.0) as u32;
    (w, h)
}

/// Apply headless-hide / windowed-show, then return the honest winit residual.
/// Never invents visibility: headless stays false even if AppKit later reports shown.
pub(super) fn apply_runtime_host_window_visibility(window: &Window, headless: bool) -> bool {
    if headless {
        window.set_visible(false);
    } else {
        window.set_visible(true);
    }
    crate::executable_smoke::ExecutableSmokeResult::window_visible_from_winit_query(
        headless,
        window.is_visible(),
    )
}

/// Park the windowed host at the top-left so MainMenu gadgets are not
/// covered by IDE/agent windows. Headless must never call this.
pub(super) fn apply_runtime_host_window_placement(window: &Window) {
    let scale = window.scale_factor().max(0.0001);
    let x = (0.0 * scale).round() as i32;
    let y = (80.0 * scale).round() as i32;
    window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    window.focus_window();
    make_host_window_key_and_accept_mouse(window);
}

/// C++ Win32 host is a foreground HWND. On macOS the winit NSWindow must be
/// key and must accept mouse, or HID clicks never become WindowEvent::MouseInput.
pub(super) fn make_host_window_key_and_accept_mouse(window: &Window) {
    window.focus_window();
    #[cfg(target_os = "macos")]
    macos_make_key_and_accept_mouse(window);
}

#[cfg(target_os = "macos")]
fn macos_make_key_and_accept_mouse(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        log::warn!("macos key-window: no window handle");
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view = appkit.ns_view.as_ptr();
    // SAFETY: ns_view comes from a valid AppKit RawWindowHandle just
    // matched above; still owned by the live winit Window.
    unsafe {
        macos_nsapp_key_window(ns_view);
    }
}

#[cfg(target_os = "macos")]
// SAFETY: ns_view must be a live NSView pointer from a valid window
// handle (enforced by callers); msg_send! targets follow AppKit
unsafe fn macos_nsapp_key_window(ns_view: *mut std::ffi::c_void) {
    use objc::runtime::{Object, Sel, YES};
    use objc::{class, msg_send, sel, sel_impl};

    if ns_view.is_null() {
        return;
    }
    let view: *mut Object = ns_view.cast();
    let ns_window: *mut Object = msg_send![view, window];
    if ns_window.is_null() {
        log::warn!("macos key-window: NSView has no NSWindow");
        return;
    }
    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    // 0 = NSApplicationActivationPolicyRegular
    let _: isize = msg_send![app, setActivationPolicy: 0isize];
    let no: objc::runtime::BOOL = objc::runtime::NO;
    let _: () = msg_send![ns_window, setIgnoresMouseEvents: no];
    let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: YES];
    let _: () = msg_send![ns_window, orderFrontRegardless];
    let nil: *mut Object = std::ptr::null_mut();
    let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
    let activate: Sel = sel!(activate);
    let responds: objc::runtime::BOOL = msg_send![app, respondsToSelector: activate];
    if responds == YES {
        let _: () = msg_send![app, activate];
    }
    log::info!("macos key-window: orderFrontRegardless + makeKeyAndOrderFront + activate");
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct NsSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct NsRect {
    origin: CgPoint,
    size: NsSize,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
// SAFETY: CoreGraphics/CoreFoundation C ABI mirrors Apple headers;
// CF types returned are released or consumed within each helper.
unsafe extern "C" {
    fn CGEventCreate(source: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGEventGetLocation(event: *mut std::ffi::c_void) -> CgPoint;
    fn CGWindowListCopyWindowInfo(option: u32, relative_window: u32) -> *mut std::ffi::c_void;
    fn CFArrayGetCount(the_array: *mut std::ffi::c_void) -> isize;
    fn CFArrayGetValueAtIndex(
        the_array: *mut std::ffi::c_void,
        idx: isize,
    ) -> *const std::ffi::c_void;
    fn CFDictionaryGetValue(
        the_dict: *const std::ffi::c_void,
        key: *const std::ffi::c_void,
    ) -> *const std::ffi::c_void;
    fn CFNumberGetValue(
        number: *const std::ffi::c_void,
        the_type: i32,
        value_ptr: *mut std::ffi::c_void,
    ) -> u8;
    fn CFStringCreateWithCString(
        alloc: *mut std::ffi::c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> *mut std::ffi::c_void;
    fn CFRelease(cf: *mut std::ffi::c_void);
}

/// Client-logical cursor if the OS pointer is inside this window's NSView.
/// Maps through AppKit convertRectToScreen — winit inner_position is wrong on
/// this host (HID at Solo Play was landing on EarthMap2).
pub(super) fn macos_cursor_client_if_in_window(window: &Window) -> Option<(f32, f32)> {
    // SAFETY: CGEventCreate(null) creates a new event reference owned by
    // us, checked below before use.
    let event = unsafe { CGEventCreate(std::ptr::null_mut()) };
    if event.is_null() {
        return None;
    }
    // SAFETY: event is a non-null CGEventRef created above; query only.
    let pt = unsafe { CGEventGetLocation(event) };
    // SAFETY: releases the reference created above exactly once, after
    // its last read.
    unsafe { CFRelease(event) };
    let size = window.inner_size();
    let scale = window.scale_factor().max(0.0001);
    let lw = (size.width as f64 / scale).max(1.0);
    let lh = (size.height as f64 / scale).max(1.0);
    // SAFETY: helper queries CGWindowList (thread-safe) and returns an
    // owned NsRect copy or None.
    let cg = unsafe { macos_own_cg_window_bounds(lw, lh) }?;
    log::info!(
        "macos cg window origin=({:.0},{:.0}) size={:.0}x{:.0} event=({:.0},{:.0})",
        cg.origin.x,
        cg.origin.y,
        cg.size.width,
        cg.size.height,
        pt.x,
        pt.y
    );
    if cg.size.width <= 1.0 || cg.size.height <= 1.0 {
        return None;
    }
    // size/scale already computed for window matching
    // Title bar sits above the Metal view; strip it using aspect of inner size.
    let title = (cg.size.height - lh * (cg.size.width / lw)).max(0.0);
    let content_h = (cg.size.height - title).max(1.0);
    let cx = (pt.x - cg.origin.x) * lw / cg.size.width;
    let cy = (pt.y - cg.origin.y - title) * lh / content_h;
    if cx >= 0.0 && cy >= 0.0 && cx < lw && cy < lh {
        Some((cx as f32, cy as f32))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
// SAFETY: pure CGWindowListCopyWindowInfo query filtered by own PID;
// every CF object created here is CFReleased on all paths below.
unsafe fn macos_own_cg_window_bounds(target_w: f64, target_h: f64) -> Option<NsRect> {
    const ON_SCREEN: u32 = 1;
    const UTF8: u32 = 0x0800_0100;
    const CF_SINT32: i32 = 3;
    const CF_FLOAT64: i32 = 6;
    let arr = CGWindowListCopyWindowInfo(ON_SCREEN, 0);
    if arr.is_null() {
        return None;
    }
    let pid = libc::getpid();
    let cstr =
        |s: &[u8]| CFStringCreateWithCString(std::ptr::null_mut(), s.as_ptr() as *const i8, UTF8);
    let key_pid = cstr(b"kCGWindowOwnerPID\0");
    let key_bounds = cstr(b"kCGWindowBounds\0");
    let key_x = cstr(b"X\0");
    let key_y = cstr(b"Y\0");
    let key_w = cstr(b"Width\0");
    let key_h = cstr(b"Height\0");
    let want_w = target_w * 0.9;
    let want_h = target_h + 32.0;
    let mut found: Option<(f64, NsRect)> = None;
    let n = CFArrayGetCount(arr);
    for i in 0..n {
        let dict = CFArrayGetValueAtIndex(arr, i);
        if dict.is_null() {
            continue;
        }
        let pid_num = CFDictionaryGetValue(dict, key_pid);
        if pid_num.is_null() {
            continue;
        }
        let mut owner: i32 = 0;
        if CFNumberGetValue(pid_num, CF_SINT32, (&raw mut owner).cast()) == 0 {
            continue;
        }
        if owner != pid {
            continue;
        }
        let bounds = CFDictionaryGetValue(dict, key_bounds);
        if bounds.is_null() {
            continue;
        }
        let read_f64 = |key: *mut std::ffi::c_void| -> Option<f64> {
            let num = CFDictionaryGetValue(bounds, key);
            if num.is_null() {
                return None;
            }
            let mut v = 0.0f64;
            if CFNumberGetValue(num, CF_FLOAT64, (&raw mut v).cast()) == 0 {
                return None;
            }
            Some(v)
        };
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            read_f64(key_x),
            read_f64(key_y),
            read_f64(key_w),
            read_f64(key_h),
        ) {
            if w > 1.0 && h > 1.0 {
                let score = (w - want_w).abs().min((w - target_w).abs())
                    + (h - want_h).abs().min((h - target_h).abs());
                if found.as_ref().map(|(s, _)| *s).unwrap_or(f64::MAX) > score {
                    found = Some((
                        score,
                        NsRect {
                            origin: CgPoint { x, y },
                            size: NsSize {
                                width: w,
                                height: h,
                            },
                        },
                    ));
                }
            }
        }
    }
    CFRelease(key_pid);
    CFRelease(key_bounds);
    CFRelease(key_x);
    CFRelease(key_y);
    CFRelease(key_w);
    CFRelease(key_h);
    CFRelease(arr);
    found.map(|(_, rect)| rect)
}

pub(super) fn device_button_to_mouse(button: u32) -> Option<MouseButton> {
    match button {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Right),
        2 => Some(MouseButton::Middle),
        _ => None,
    }
}

pub(super) fn update_iconic_state_and_wake_audio(window: &Window, minimized: &mut bool) {
    let was_minimized = *minimized;
    *minimized = query_window_is_iconic(window, *minimized);

    if was_minimized && !*minimized {
        info!("Window exited iconic/minimized state");
        with_subsystem_mut::<AudioManagerSubsystem, _>(|audio| {
            audio.wake_after_iconic_return();
        });
    } else if !was_minimized && *minimized {
        info!("Window entered iconic/minimized state");
    }
}

pub(super) fn should_exit_for_smoke_test(
    smoke_test: bool,
    state: GameState,
    startup_progress: f32,
    exiting_pending: bool,
) -> bool {
    smoke_test && matches!(state, GameState::Menu) && startup_progress >= 1.0 && !exiting_pending
}

#[cfg(feature = "internal")]
pub mod parity_test_support {
    use super::GameState;
    use crate::ui::Screen;

    /// Lightweight state-machine model used by parity tests.
    ///
    /// The real engine constructor is too heavy for fast integration tests, so this
    /// harness mirrors the transition side effects that matter for startup, match
    /// start, exit-to-menu, and quit deduplication coverage.
    #[derive(Debug, Clone)]
    pub struct StateMachineParityHarness {
        pub(crate) current_state: GameState,
        pub(crate) pending_state: Option<GameState>,
        pub(crate) ui_screen: Option<Screen>,
        pub(crate) game_paused: bool,
        pub(crate) game_logic_paused: bool,
        pub(crate) match_over: bool,
        pub(crate) victory_summary_present: bool,
        pub(crate) selected_objects: Vec<u32>,
        pub(crate) quit_requests_emitted: usize,
        pub(crate) menu_world_frames_rendered: u32,
    }

    impl Default for StateMachineParityHarness {
        fn default() -> Self {
            Self {
                current_state: GameState::Menu,
                pending_state: None,
                ui_screen: Some(Screen::MainMenu),
                game_paused: false,
                game_logic_paused: false,
                match_over: false,
                victory_summary_present: false,
                selected_objects: Vec::new(),
                quit_requests_emitted: 0,
                menu_world_frames_rendered: 0,
            }
        }
    }

    impl StateMachineParityHarness {
        pub fn current_state(&self) -> GameState {
            self.current_state
        }

        pub fn pending_state(&self) -> Option<GameState> {
            self.pending_state
        }

        pub fn ui_screen(&self) -> Option<Screen> {
            self.ui_screen
        }

        pub fn game_paused(&self) -> bool {
            self.game_paused
        }

        pub fn game_logic_paused(&self) -> bool {
            self.game_logic_paused
        }

        pub fn match_over(&self) -> bool {
            self.match_over
        }

        pub fn victory_summary_present(&self) -> bool {
            self.victory_summary_present
        }

        pub fn selected_objects(&self) -> &[u32] {
            &self.selected_objects
        }

        pub fn quit_requests_emitted(&self) -> usize {
            self.quit_requests_emitted
        }

        pub fn set_loading_state(&mut self) {
            self.current_state = GameState::Loading;
            self.pending_state = None;
            self.ui_screen = Some(Screen::Loading);
        }

        pub fn set_dirty_play_state(&mut self) {
            self.current_state = GameState::InGame;
            self.pending_state = None;
            self.ui_screen = Some(Screen::GameHUD);
            self.game_paused = true;
            self.game_logic_paused = true;
            self.match_over = true;
            self.victory_summary_present = true;
            self.selected_objects = vec![101, 202, 303];
        }

        pub fn complete_startup_loading_to_menu(&mut self) {
            self.transition_to_state(GameState::Menu);
        }

        pub fn complete_new_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn complete_load_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn return_to_main_menu_after_match(&mut self) {
            self.selected_objects.clear();
            self.game_paused = false;
            self.game_logic_paused = false;
            self.match_over = false;
            self.victory_summary_present = false;
            self.pending_state = None;
            self.transition_to_state(GameState::Menu);
        }

        pub fn request_quit(&mut self) -> bool {
            if self.current_state == GameState::Exiting
                || self.pending_state == Some(GameState::Exiting)
            {
                return false;
            }

            self.pending_state = Some(GameState::Exiting);
            self.quit_requests_emitted = self.quit_requests_emitted.saturating_add(1);
            true
        }

        pub fn apply_pending_state_change(&mut self) {
            if let Some(new_state) = self.pending_state.take() {
                self.transition_to_state(new_state);
            }
        }

        fn transition_to_state(&mut self, new_state: GameState) {
            match new_state {
                GameState::Initializing => {
                    self.ui_screen = Some(Screen::Loading);
                }
                GameState::Menu => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::MainMenu);
                    self.menu_world_frames_rendered = 0;
                }
                GameState::Loading => {
                    self.ui_screen = Some(Screen::Loading);
                }
                GameState::InGame => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::GameHUD);
                }
                GameState::Paused => {
                    self.game_paused = true;
                    self.game_logic_paused = true;
                    self.ui_screen = Some(Screen::PauseMenu);
                }
                GameState::Victory | GameState::Defeat => {
                    self.game_paused = true;
                    self.game_logic_paused = true;
                    self.match_over = true;
                    self.victory_summary_present = true;
                    self.ui_screen = Some(Screen::GameHUD);
                }
                GameState::Exiting => {
                    self.ui_screen = None;
                }
            }

            self.current_state = new_state;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScriptCameraShaker {
    pub(crate) epicenter: Vec3,
    pub(crate) radius: f32,
    pub(crate) duration_seconds: f32,
    pub(crate) elapsed_seconds: f32,
    /// C++ `Add_Camera_Shake` power*PI/180 intensity (radians).
    pub(crate) intensity: f32,
    /// Per-axis start omega in [12.5, 15] revolutions/s (radians).
    pub(crate) omega: Vec3,
    pub(crate) phi: Vec3,
    pub(crate) rng_seed: u32,
}

impl ScriptCameraShaker {
    pub(super) fn new(
        epicenter: Vec3,
        radius: f32,
        duration_seconds: f32,
        amplitude_degrees: f32,
    ) -> Self {
        // Deterministic stand-in for C++ WWMath::Random_Float(MIN_OMEGA, MAX_OMEGA)
        // and Random_Float(0, 360deg). Frequency stays in the 12.5-15Hz band.
        let seed = (epicenter.x.to_bits())
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(epicenter.y.to_bits().rotate_left(7))
            .wrapping_add(epicenter.z.to_bits().rotate_left(13))
            .wrapping_add(amplitude_degrees.to_bits());
        let unit = |lane: u32| -> f32 {
            let mut x = seed ^ lane.wrapping_mul(0x85EB_CA6B);
            x ^= x >> 16;
            x = x.wrapping_mul(0x7FEB_352D);
            x ^= x >> 15;
            x = x.wrapping_mul(0x846C_A68B);
            x ^= x >> 16;
            (x >> 8) as f32 / ((1u32 << 24) as f32)
        };
        let min_omega = 12.5 * std::f32::consts::TAU;
        let max_omega = 15.0 * std::f32::consts::TAU;
        let omega_at = |lane: u32| min_omega + unit(lane) * (max_omega - min_omega);
        Self {
            epicenter,
            radius: radius.max(0.01),
            duration_seconds: duration_seconds.max(0.01),
            elapsed_seconds: 0.0,
            intensity: amplitude_degrees.abs() * std::f32::consts::PI / 180.0,
            omega: Vec3::new(omega_at(1), omega_at(2), omega_at(3)),
            phi: Vec3::new(
                unit(4) * std::f32::consts::TAU,
                unit(5) * std::f32::consts::TAU,
                unit(6) * std::f32::consts::TAU,
            ),
            rng_seed: seed,
        }
    }
}

pub(crate) struct StartupLoadResult {
    pub(crate) game_logic: GameLogic,
    pub(crate) loaded_map_name: Option<String>,
    pub(crate) start_in_menu: bool,
    pub(crate) map_requested_from_cli: bool,
    pub(crate) replay_requested: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StartupNewGameDispatch {
    pub(crate) game_mode_code: i32,
    pub(crate) game_mode: GameMode,
    pub(crate) difficulty_code: i32,
    pub(crate) difficulty: GameDifficulty,
    pub(crate) rank_points: i32,
    pub(crate) max_fps: Option<i32>,
}

/// Fully resolved Main-owned match-start payload.
///
/// Replacing the old positional `(mode, faction, map, skirmish)` tuple keeps a
/// selected Campaign/Challenge PlayerTemplate attached to every NewGame drain.
/// Direct runtime/UI callers must use [`Self::without_player_template`] so a
/// normal or skirmish start cannot inherit stale selection state.
#[derive(Debug, Clone)]
pub(super) struct HostStartRequest {
    pub(super) mode: GameMode,
    pub(super) faction: String,
    pub(super) map: String,
    pub(super) skirmish: Option<crate::skirmish_config::SkirmishMatchConfig>,
    pub(super) player_template: Option<crate::game_logic::PlayerTemplateIdentity>,
}

impl HostStartRequest {
    pub(super) fn without_player_template(
        mode: GameMode,
        faction: String,
        map: String,
        skirmish: Option<crate::skirmish_config::SkirmishMatchConfig>,
    ) -> Self {
        Self {
            mode,
            faction,
            map,
            skirmish,
            player_template: None,
        }
    }

    pub(super) fn with_player_template(
        mode: GameMode,
        faction: String,
        map: String,
        skirmish: Option<crate::skirmish_config::SkirmishMatchConfig>,
        player_template: crate::game_logic::PlayerTemplateIdentity,
    ) -> Self {
        Self {
            mode,
            faction,
            map,
            skirmish,
            player_template: Some(player_template),
        }
    }
}

/// UI start parked after `GameState::Loading` so the runtime-host can publish
/// `state=Loading` before the blocking `host_load_map_or_default` call.
///
/// C++ `GameLogic::startNewGame` also enters the load screen before `loadMap`.
/// Headless smoke previously never left Menu because `start_game_from_ui` is
/// synchronous and Lone Eagle `load_map` can stall the same command. The next
/// Loading tick finishes the parked start (still calls `load_map`; does not
/// skip it). Physical five-flag `playable_claim` is unchanged.
#[derive(Debug, Clone)]
pub(super) struct PendingMatchStart {
    pub(super) request: HostStartRequest,
    pub(super) interactive_start_from_menu: bool,
}

/// Main extraction of the generation-matched GameClient campaign descriptor.
/// The bridge stays responsible for publication/lifetime matching; this type
/// owns only the authoritative start fields that must survive to GameLogic.
#[derive(Debug, Clone, Default)]
pub(super) struct CampaignLaunchStartOverrides {
    pub(super) map: Option<String>,
    pub(super) faction: Option<String>,
    pub(super) player_template: Option<crate::game_logic::PlayerTemplateIdentity>,
}

/// Parsed `FIRE_WEAPON` data retained while the player chooses a target.
///
/// C++ keeps both fields on its pending CommandButton and selects the final
/// message type from `ATTACK_OBJECTS_POSITION` at click time.  Retaining the
/// raw option bits here prevents an object click from erasing that behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingWeaponCommand {
    pub(super) weapon_slot: crate::command_system::WeaponSlot,
    pub(super) max_shots_to_fire: i32,
    pub(super) options: u32,
}

impl PendingWeaponCommand {
    /// C++ `USES_MINE_CLEARING_WEAPONSET`: the control bar sends
    /// `MSG_SET_MINE_CLEARING_DETAIL` to the vetted selection before it arms
    /// this otherwise ordinary `FIRE_WEAPON` target command.
    pub(super) fn uses_mine_clearing_weapon_set(&self) -> bool {
        const USES_MINE_CLEARING_WEAPONSET: u32 = 0x0020_0000;
        self.options & USES_MINE_CLEARING_WEAPONSET != 0
    }

    /// C++ `ATTACK_OBJECTS_POSITION`: an object validates the click, but the
    /// weapon attacks the terrain location under that object rather than its
    /// object ID.
    pub(super) fn attacks_object_position(&self) -> bool {
        const ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
        self.options & ATTACK_OBJECTS_POSITION != 0
    }
    /// C++ `ALLOW_MINE_TARGET` (`Command.h` bit 11).
    pub(super) fn allows_mine_target(&self) -> bool {
        const ALLOW_MINE_TARGET: u32 = 0x0000_0800;
        self.options & ALLOW_MINE_TARGET != 0
    }

    /// C++ `ALLOW_SHRUBBERY_TARGET` (`Command.h` bit 4).
    pub(super) fn allows_shrubbery_target(&self) -> bool {
        const ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
        self.options & ALLOW_SHRUBBERY_TARGET != 0
    }
}

/// Parsed `COMBATDROP` targeting data retained while the player chooses a
/// target.
///
/// A retail Combat Drop button may accept both objects and map positions.
/// C++ resolves that only at click time: an eligible object wins, otherwise a
/// position click is used.  Keep the original bit field instead of reducing
/// the command to one target kind while it is armed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingCombatDropCommand {
    pub(super) options: u32,
}

impl PendingCombatDropCommand {
    const NEED_OBJECT_TARGET: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;
    const NEED_TARGET_POSITION: u32 = 0x0000_0020;

    /// Non-ControlBar callers only have a location-valued host command, so
    /// they retain the pre-existing location-only behavior rather than
    /// claiming an unparsed object-target capability.
    pub(super) const fn position_only() -> Self {
        Self {
            options: Self::NEED_TARGET_POSITION,
        }
    }

    /// C++ `COMMAND_OPTION_NEED_OBJECT_TARGET`: enemy, neutral, or ally.
    pub(super) fn accepts_object_target(&self) -> bool {
        self.options & Self::NEED_OBJECT_TARGET != 0
    }

    pub(super) fn accepts_position_target(&self) -> bool {
        self.options & Self::NEED_TARGET_POSITION != 0
    }
}

/// Map-click command residual armed by ControlBar buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingMapCommand {
    AttackMove,
    /// C++ GuardMode carried from the arming command button.
    Guard(crate::game_logic::GuardMode),
    SetRallyPoint,
    /// Chinook combat drop residual retaining its parsed target options.
    CombatDrop(PendingCombatDropCommand),
    /// Armed superweapon / special power residual awaiting map click.
    SpecialPower(crate::command_system::SpecialPowerType),
    /// A `FIRE_WEAPON` CommandButton retains its exact slot, shot limit, and
    /// target-selection options until the player chooses an object or map
    /// position.
    Weapon(PendingWeaponCommand),
    /// Retail PLACE_BEACON residual awaiting map click.
    PlaceBeacon,
    /// Unit special-ability residual awaiting object/map click.
    UnitAbility(PendingUnitAbility),
}

impl PendingMapCommand {
    /// Armed CommandButton option bits when the pending command retains them.
    /// Attack-move / guard / rally have no pick-widening bits.
    pub(super) fn command_option_bits(&self) -> Option<u32> {
        match self {
            Self::Weapon(weapon) => Some(weapon.options),
            Self::CombatDrop(combat_drop) => Some(combat_drop.options),
            _ => None,
        }
    }
}

/// What the active left-button gesture must do once it is released.
///
/// C++ routes physical world clicks through `PlaceEventTranslator`,
/// `GUICommandTranslator`, `SelectionTranslator`, and `CommandTranslator` in
/// that order.  Main receives the OS edge directly, so it retains this tiny
/// per-gesture decision rather than treating alternate mouse as a blind button
/// swap.  In particular, an armed map/build command stays on LMB in both
/// mouse layouts, while classic (non-alternate) LMB context commands wait long
/// enough to yield to a selection-box drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LeftMouseReleaseBehavior {
    #[default]
    Selection,
    ContextCommand,
    /// A target/action was already consumed on the press edge, so its release
    /// must not clear the selection or issue a second command.
    Suppress,
}

/// ControlBar unit ability that needs a target click residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingUnitAbility {
    Hijack,
    Sabotage,
    CaptureBuilding,
    SnipeVehicle,
    PlantTimedDemoCharge,
    PlantRemoteDemoCharge,
    StealCashHack,
    DisableVehicleHack,
    HackerDisableBuilding,
    DisguiseAsVehicle,
    PlantBoobyTrap,
    ConvertToCarbomb,
    /// Dozer/Worker repair residual awaiting damaged structure click.
    Repair,
}

/// Evidence for retail windowed sit-through (`wnd_widget_tree_nav` /
/// interactive gameplay). Latched **only** via
/// [`CnCGameEngine::handle_mouse_button_input`] (physical winit `MouseInput` or
/// winit-equivalent inject that re-enters that path after a real gadget hit /
/// RMB release with selection).
///
/// Host control cmds must **not** call `note_menu_wnd_click` /
/// `note_gameplay_order` directly. Scripted `drive_os_wnd_*` and headless
/// soft UI cannot manufacture this evidence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct InteractivePlayabilityEvidence {
    /// A physical left click was hit-tested and consumed by a visible shell WND
    /// widget while the engine was in its menu state.
    pub(crate) menu_wnd_click: bool,
    /// That click (or a later one) hit a MainMenu → Skirmish / Start gadget,
    /// not Parent/Ruler/Options chrome.
    pub(crate) skirmish_path: bool,
    /// That menu interaction subsequently started an offline match through the
    /// normal `start_game_from_ui` authority path.
    pub(crate) match_started_from_menu_wnd: bool,
    /// A physical context-click issued an order after that match began.  Selection
    /// alone is intentionally insufficient: this proves a player can command a
    /// unit, not merely move the pointer over the HUD.
    pub(crate) gameplay_order: bool,
    /// A physical Control Bar `DozerConstruct` button was accepted for a live
    /// local dozer/worker in a visible offline match.  This is deliberately
    /// distinct from `CommandSourceType::FromUser`: injected WND input also
    /// uses that legacy source type and must not satisfy this proof.
    pub(crate) physical_control_bar_construct_armed: bool,
    /// A physical Control Bar production request was accepted after the above
    /// construct arm in the same session.  This is the narrow build-and-
    /// produce acceptance condition, not a broad runtime-host command flag.
    pub(crate) physical_build_and_produce: bool,
    /// A carrier selected by a confirmed physical Gather order subsequently
    /// deposited a positive carried-supply amount for the local player in a
    /// visible offline match. Passive income and untracked carriers cannot
    /// advance this proof.
    pub(crate) physical_gather_resources: bool,
    /// An explicit PopupSaveLoad save confirmation completed through Main's
    /// snapshot authority after a real physical WND mouse event in a visible
    /// offline match.
    pub(crate) physical_popup_save_confirmation_succeeded: bool,
    /// A physical PopupSaveLoad load confirmation completed after the physical
    /// save confirmation above.  This proves a player can save, load, and
    /// continue through the real Rust-owned popup/authority path.
    pub(crate) physical_save_load_continue: bool,
}

impl InteractivePlayabilityEvidence {
    pub(super) fn note_menu_wnd_click(
        &mut self,
        windowed: bool,
        wnd_consumed: bool,
        hit_widget: bool,
    ) {
        if windowed && wnd_consumed && hit_widget {
            self.menu_wnd_click = true;
            log::info!(
                "InteractivePlayabilityEvidence: latched menu_wnd_click (windowed={windowed} consumed={wnd_consumed} hit={hit_widget})"
            );
        } else {
            log::debug!(
                "InteractivePlayabilityEvidence: menu_wnd_click miss windowed={windowed} consumed={wnd_consumed} hit={hit_widget}"
            );
        }
    }

    pub(super) fn note_skirmish_path_gadget(&mut self, windowed: bool, gadget_name: &str) {
        if windowed
            && crate::executable_smoke::ExecutableSmokeResult::wnd_nav_gadget_is_skirmish_path(
                gadget_name,
            )
        {
            self.skirmish_path = true;
        }
    }

    pub(super) fn note_offline_match_started(&mut self, was_menu: bool, offline_mode: bool) {
        if self.menu_wnd_click && self.skirmish_path && was_menu && offline_mode {
            self.match_started_from_menu_wnd = true;
        }
    }

    pub(super) fn note_gameplay_order(&mut self, windowed: bool, had_selection: bool) {
        if windowed && self.match_started_from_menu_wnd && had_selection {
            self.gameplay_order = true;
        }
    }

    /// Record an already-validated physical DozerConstruct arm.
    ///
    /// Callers must prove the input was a real OS mouse event and that the
    /// selected source is a local live dozer/worker before reaching this
    /// method.  Keeping those authority checks outside this sticky evidence
    /// type avoids teaching it to infer gameplay state from UI labels.
    pub(super) fn note_control_bar_construct_arm(&mut self, physical: bool) {
        if physical {
            self.physical_control_bar_construct_armed = true;
        }
    }

    /// Record an already-validated physical production queue request.
    ///
    /// Ordering is intentional: a physical production request before a valid
    /// physical construct arm cannot satisfy the build-and-produce condition.
    pub(super) fn note_control_bar_production(&mut self, physical: bool) {
        if physical && self.physical_control_bar_construct_armed {
            self.physical_build_and_produce = true;
        }
    }

    /// Whether this session has completed the physical Control Bar
    /// build-and-produce proof.
    pub(super) fn build_and_produce_complete(self) -> bool {
        self.physical_build_and_produce
    }

    /// Record an already-validated physical supply drop-off.
    ///
    /// The caller must have matched the drop-off to a carrier from an accepted
    /// physical Gather command, verified a positive carried-supply amount, and
    /// verified the visible offline/local-player conditions. Keeping that
    /// gameplay authority outside this sticky evidence type prevents passive or
    /// runtime-host income from being inferred as physical input.
    pub(super) fn note_physical_gather_resources(&mut self, physical: bool) {
        if physical {
            self.physical_gather_resources = true;
        }
    }

    /// Whether this session has completed the physical Gather → drop-off proof.
    pub(super) fn gather_resources_complete(self) -> bool {
        self.physical_gather_resources
    }

    /// Record a Main-authority save success that already passed the physical
    /// Popup confirmation and visible-offline-match checks.
    pub(super) fn note_popup_save_confirmation_succeeded(&mut self, physical: bool) {
        if physical {
            self.physical_popup_save_confirmation_succeeded = true;
        }
    }

    /// Record a Main-authority load success after a physical Popup confirmation.
    ///
    /// Ordering is intentional: a physical load alone, or one following a
    /// runtime-host/injected save, cannot claim save/load continuation.
    pub(super) fn note_popup_load_confirmation_succeeded(&mut self, physical: bool) {
        if physical && self.physical_popup_save_confirmation_succeeded {
            self.physical_save_load_continue = true;
        }
    }

    /// Whether this session has completed physical PopupSaveLoad save → load
    /// continuation through Main's snapshot authority.
    pub(super) fn save_load_continue_complete(self) -> bool {
        self.physical_save_load_continue
    }

    /// The WND-navigation component of the retail claim requires the complete
    /// menu-to-match chain, rather than a broad sticky "some gadget was hovered"
    /// bit from the GUI singleton.
    pub(super) fn wnd_menu_to_match_complete(self) -> bool {
        self.menu_wnd_click && self.skirmish_path && self.match_started_from_menu_wnd
    }

    pub(super) fn gameplay_complete(self) -> bool {
        self.match_started_from_menu_wnd && self.gameplay_order
    }
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod evidence_tests;

/// Main C&C game engine with full RTS functionality - restructured to match C++ SAGE architecture
pub struct CnCGameEngine {
    pub(crate) window: Arc<Window>,
    #[allow(dead_code)] // C++ parity: stored for future command-line query access
    pub(crate) command_line: Arc<CommandLineArgs>,

    // C++ SAGE equivalent rendering subsystems
    pub(crate) graphics_system: GraphicsSystem,
    pub(crate) render_pipeline: RenderPipeline,

    // Platform message handling
    pub(crate) message_processor: WindowMessageProcessor,

    // Audio system
    #[allow(dead_code)] // C++ parity: audio stream handle kept alive to prevent drop
    pub(crate) audio_output: Option<OutputStream>,
    pub(crate) audio_handle: Option<OutputStreamHandle>,
    pub(crate) background_music: Option<Sink>,
    pub(crate) sound_effects: Vec<Sink>,
    pub(crate) ui_sound_cache: HashMap<String, Arc<[u8]>>,

    // Game state machine - matches C++ GameEngine m_quitting and state management
    pub(crate) current_state: GameState,
    pub(crate) pending_state: Option<GameState>,
    pub(crate) startup_load_state: StartupLoadState,
    pub(crate) startup_target_state: Option<GameState>,
    pub(crate) startup_start_in_menu: bool,
    pub(crate) last_loading_title_update: Option<Instant>,
    pub(crate) startup_last_reported_progress: f32,
    pub(crate) startup_loading_phase: String,
    pub(crate) startup_last_progress_change_at: Instant,
    pub(crate) startup_last_stall_warning_at: Option<Instant>,
    pub(crate) startup_stall_events: u32,
    pub(crate) startup_max_stall_duration: Duration,
    pub(crate) startup_health_summary_logged: bool,
    pub(crate) last_caustic_warmup_attempt: Option<Instant>,
    pub(crate) loading_overlay_active: bool,
    #[cfg(feature = "game_client")]
    pub(crate) active_load_screen: Option<game_client::gui::load_screen::LoadScreenKind>,
    pub(crate) shell_menu_active: bool, // C++ parity: Shell::push("Menus/MainMenu.wnd") / Shell::pop()

    // Game client — C++ parity: TheGameClient singleton, wired into Main's frame loop
    // for drawable updates and display draw. Full GameClient::update() OS-input path
    // is not used (Main owns input→commands); drawables always tick with the frame.
    #[cfg(feature = "game_client")]
    pub(crate) game_client: game_client::core::game_client::GameClient,
    /// ControlBar selection panel (portrait + health). Presentation-fed; WND load optional.
    #[cfg(feature = "game_client")]
    pub(crate) control_bar: game_client::gui::control_bar::ControlBar,

    // Game state
    pub(crate) game_logic: GameLogic,
    /// Map-lifetime immutable terrain payload shared by presentation frames.
    ///
    /// C++ keeps its `WorldHeightMap` alive in `W3DTerrainVisual` for the map
    /// lifetime. This host cache mirrors that ownership boundary so handing a
    /// frame to the render pipeline does not clone the full height/blend data.
    pub(crate) presentation_terrain_cache: PresentationTerrainCache,
    /// Immutable presentation feed for client/render after last logic step.
    pub(crate) last_presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Runtime-only identity epoch for direct host Drawable associations.
    ///
    /// Object IDs are reused by reset, map install, and restore.  This epoch is
    /// intentionally not part of a snapshot: C++ reconstructs Drawable
    /// associations and clears volatile shroud history at those boundaries.
    pub(crate) host_direct_visual_world_epoch: u64,
    /// Wave 842: host-owned match mode residual set after a successful match
    /// world start or restore.
    /// Prefer over live GameLogic::game_mode when presentation freeze is missing.
    pub(crate) host_match_game_mode: Option<GameMode>,
    /// Wave 843: host-owned match map / local player / AI difficulty residuals.
    pub(crate) host_match_map_name: Option<String>,
    pub(crate) host_match_local_player_id: Option<u32>,
    pub(crate) host_match_ai_difficulty: Option<crate::ai::AIDifficulty>,
    /// Wave 844: host-owned sim timing residuals (prefer over live GameLogic probes).
    pub(crate) host_match_visual_speed: Option<f32>,
    pub(crate) host_match_time_frozen: Option<bool>,
    pub(crate) host_match_total_play_time: Option<f32>,
    pub(crate) host_match_logic_frame: Option<u32>,
    pub(crate) host_match_logic_steps: Option<(u32, bool, f32)>,
    pub(crate) host_match_in_replay: Option<bool>,
    /// Wave 845: host-owned shell/team residuals for presentation_or_boot peels.
    pub(crate) host_match_in_shell: Option<bool>,
    pub(crate) host_match_local_team: Option<crate::game_logic::Team>,
    /// Wave 846: host-owned diplomacy / template / sciences residuals.
    pub(crate) host_match_diplomacy_players:
        Option<Vec<crate::presentation_frame::PresentationPlayerInfo>>,
    pub(crate) host_match_known_template_names: Option<Vec<String>>,
    pub(crate) host_match_unlocked_sciences: Option<std::collections::HashMap<u32, Vec<String>>>,
    /// Wave 847: host-owned camera-follow residuals for presentation_or_boot peels.
    pub(crate) host_match_camera_follow_active: Option<bool>,
    pub(crate) host_match_camera_follow_position: Option<[f32; 3]>,
    /// Wave 913: last camera-follow object residual (skip redundant authority writes).
    pub(crate) host_match_camera_follow_id: Option<Option<crate::game_logic::ObjectId>>,
    /// C++ W3DView followFactor; -1 when unlocked.
    pub(crate) camera_follow_factor: f32,
    /// Wave 848: host-owned local train producers residual (barracks / other).
    pub(crate) host_match_local_barracks_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_producer_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_unfinished_producer_ids: Option<Vec<crate::game_logic::ObjectId>>,
    pub(crate) host_match_local_team_sample_pos: Option<[f32; 3]>,
    /// Wave 849: host-owned match outcome residuals (victory peels).
    pub(crate) host_match_over: Option<bool>,
    pub(crate) host_match_victory_label: Option<String>,
    /// Meaningful when `host_match_over == Some(true)`: None=draw, Some(id)=winner.
    pub(crate) host_match_victory_winner: Option<Option<u32>>,
    pub(crate) host_match_victory_summary: Option<crate::game_logic::VictorySummary>,
    /// Wave 850: host-owned selection residual (peels player_selected_objects boot dual-read).
    pub(crate) host_match_selected_ids: Option<Vec<crate::game_logic::ObjectId>>,
    /// Wave 851: host-owned alive-object residual (peels object_is_alive boot dual-read).
    pub(crate) host_match_alive_object_ids: Option<std::collections::HashSet<u32>>,
    /// Wave 852: host-owned purchasable science residual per player.
    pub(crate) host_match_purchasable_sciences:
        Option<std::collections::HashMap<u32, std::collections::HashSet<String>>>,
    /// Wave 868: host-owned local science purchase points residual.
    pub(crate) host_match_local_science_purchase_points: Option<i32>,
    /// Wave 921: local supplies residual (presentation stamp; supplies floor peel).
    pub(crate) host_match_local_supplies: Option<u32>,
    /// Wave 854/857: host-owned special-power-ready object residual (unified scan stamp).
    pub(crate) host_match_special_power_ready_ids: Option<std::collections::HashSet<u32>>,
    /// Wave 855: boot victory condition residual (single evaluate stamp).
    /// None = not stamped; Some(None) = no winner yet; Some(Some(cond)) = outcome.
    pub(crate) host_match_boot_victory_condition:
        Option<Option<crate::game_logic::VictoryCondition>>,
    /// Wave 911: per-frame legal-build residual cache (construct pad scan peel).
    pub(crate) host_legal_build_cache_frame: Option<u32>,
    pub(crate) host_legal_build_cache:
        std::collections::HashMap<(crate::game_logic::Team, i32, i32, u64, u32), u32>,
    /// Wave 858: host-owned script camera default residuals.
    pub(crate) host_match_script_camera_max_height: Option<f32>,
    pub(crate) host_match_script_camera_pitch: Option<f32>,
    /// Wave 861: host-owned multiplayer residual (presentation dual-read peel).
    pub(crate) host_match_in_multiplayer: Option<bool>,
    /// Wave 862: host-owned world bounds residual (min, max).
    pub(crate) host_match_world_bounds: Option<(glam::Vec3, glam::Vec3)>,
    /// Wave 863: host-owned first-opponent residual (debug victory hotkey peel).
    pub(crate) host_match_first_opponent_id: Option<Option<u32>>,
    /// Optional GameWorld shadow session (stable ObjectId→EntityId).
    /// Production default ON (`GENERALS_GAMEWORLD_SHADOW=0` to opt out).
    /// Last-writer for HP/cash/pose/targets/move; not sole GameWorld authority yet.
    pub(crate) gameworld_shadow: Option<crate::gameworld_shadow::GameWorldShadow>,
    /// Observe-path entity count from GameWorld presentation view after coupled tick
    /// (architecture residual: GameWorld → presentation without Main dual-read).
    pub(crate) last_gameworld_presentation_entity_count: usize,
    /// Last presentation-overlaid UI state (selection health/minimap identity retained
    /// after render build so consumers are not dropped each frame).
    pub(crate) last_ui_state: Option<GameUIState>,
    pub(crate) resource_manager: ResourceManager,
    pub(crate) save_file_manager: SaveFileManager,

    // Camera system
    pub(crate) camera_position: Vec3,
    pub(crate) camera_target: Vec3,
    /// C++ `W3DView::m_cameraConstraint` union of scripted pans
    /// (`W3DView.cpp:3097-3212`). `(lo_x, hi_x, lo_z, hi_z)` in live Y-up.
    pub(crate) scripted_camera_constraint_widen: Option<(f32, f32, f32, f32)>,
    pub(crate) camera_zoom: f32,
    pub(crate) camera_zoom_target: Option<f32>,
    pub(crate) camera_zoom_start: f32,
    pub(crate) camera_zoom_duration: f32,
    pub(crate) camera_zoom_elapsed: f32,
    pub(crate) camera_zoom_ease_in: f32,
    pub(crate) camera_zoom_ease_out: f32,
    pub(crate) camera_orbit_distance: f32,
    pub(crate) camera_pitch_radians: f32,
    pub(crate) camera_pitch_target: Option<f32>,
    pub(crate) camera_pitch_start: f32,
    pub(crate) camera_pitch_duration: f32,
    pub(crate) camera_pitch_elapsed: f32,
    pub(crate) camera_pitch_ease_in: f32,
    pub(crate) camera_pitch_ease_out: f32,
    /// C++ `W3DView::m_FXPitch`. 0 = look flat, 1 = normal, >1 = look down.
    pub(crate) camera_fx_pitch: f32,
    pub(crate) camera_yaw_radians: f32,
    pub(crate) camera_yaw_target: Option<f32>,
    pub(crate) camera_yaw_start: f32,
    pub(crate) camera_yaw_duration: f32,
    pub(crate) camera_yaw_elapsed: f32,
    pub(crate) camera_yaw_ease_in: f32,
    pub(crate) camera_yaw_ease_out: f32,
    pub(crate) camera_shake_offset: Vec3,
    /// C++ CameraShakerSystem Compute_Rotations (pitch/yaw/roll radians).
    pub(crate) camera_shake_rotation: Vec3,
    pub(crate) screen_shake_intensity: f32,
    pub(crate) screen_shake_angle_cos: f32,
    pub(crate) screen_shake_angle_sin: f32,
    pub(crate) script_camera_shakers: Vec<ScriptCameraShaker>,
    pub(crate) script_fps_limit: Option<u32>,
    pub(crate) script_fps_limit_last_tick: Option<Instant>,
    pub(crate) camera_slave_mode: Option<CameraSlaveModeRequest>,
    pub(crate) view_matrix: Mat4,
    pub(crate) projection_matrix: Mat4,

    // Input state
    pub(crate) keys_pressed: HashSet<Key>,
    pub(crate) mouse_position: (f32, f32),
    /// True after a real OS/inject cursor move. Boot default (0,0) must not
    /// edge-scroll the camera off the map (C++ uses the live Win32 cursor).
    pub(crate) mouse_cursor_seen: bool,

    pub(crate) mouse_world_position: Vec3,
    /// Last applied context cursor residual (avoid spam set_cursor).
    pub(crate) last_context_cursor: Option<&'static str>,
    /// EVA LOWPOWER residual edge counter.
    pub(crate) last_eva_low_power_count: u32,
    pub(crate) last_eva_insufficient_funds_count: u32,
    pub(crate) last_eva_base_under_attack_count: u32,
    pub(crate) last_eva_ally_under_attack_count: u32,
    /// LogicFrame whose EvaAlerts were already pushed to chat/HUD.
    /// Same freeze is re-applied every render until the next logic tick.
    pub(crate) last_applied_eva_alert_frame: Option<u32>,
    /// C++ sticky waypoint mode residual (Alt hold still works; Z toggles).
    pub(crate) sticky_waypoint_mode: bool,
    /// Sticky auto-attack residual (Ctrl+Shift+A): convert plain moves to attack-move.
    pub(crate) sticky_auto_attack: bool,
    /// C++ `GlobalData::m_useAlternateMouse`, consumed by Main's sole
    /// AuthorityOnly physical world-input path.  GameClient keeps its own
    /// legacy consumers; live Options updates cross the typed host bridge.
    pub(crate) use_alternate_mouse: bool,
    pub(crate) is_dragging: bool,
    pub(crate) selection_start: Option<Vec3>,
    /// Screen-space drag origin for selection box overlay residual.
    pub(crate) selection_start_screen: Option<(f32, f32)>,
    pub(crate) last_click_time: Option<Instant>,
    pub(crate) last_click_position: Option<(f32, f32)>,
    pub(crate) last_right_click_time: Option<Instant>,
    pub(crate) last_right_click_position: Option<Vec3>,
    pub(crate) left_click_release_behavior: LeftMouseReleaseBehavior,
    /// C++ `SelectionTranslator::m_displayedMaxWarning` / `setDisplayedMaxWarning`.
    pub(crate) displayed_max_selection_warning: bool,
    /// Physical provenance for a classic-layout LMB context gesture.  Both
    /// edges must be physical before the existing gameplay evidence path may
    /// treat its resulting command as player input.
    pub(crate) lmb_context_started_physically: bool,
    pub(crate) is_windowed: bool,
    pub(crate) rmb_scroll_anchor: Option<(f32, f32)>,
    pub(crate) is_rmb_scrolling: bool,
    /// C++ OptionsMenu `MoveScrollAnchor` preference, consumed by Main's
    /// AuthorityOnly RMB camera drag path rather than legacy TheInGameUI.
    pub(crate) move_rmb_scroll_anchor: bool,
    /// C++ OptionsMenu `DrawScrollAnchor` preference. This is a transient
    /// presentation choice for Main's AuthorityOnly RMB drag overlay, never
    /// authoritative simulation or savegame state.
    pub(crate) draw_rmb_scroll_anchor: bool,
    /// Evidence-only provenance for the active RMB gesture. A gather proof
    /// requires the press and release to both be real OS mouse input; injected
    /// press/release pairs still execute normal gameplay but cannot qualify.
    pub(crate) rmb_scroll_started_physically: bool,
    /// C++ `SelectionTranslator::m_lastClick` / `m_deselectFeedbackAnchor` /
    /// `m_deselectDownCameraPosition` for the RMB click-vs-look-at gate.
    pub(crate) rmb_deselect_down_at: Option<Instant>,
    pub(crate) rmb_deselect_down_screen: Option<(f32, f32)>,
    pub(crate) rmb_deselect_down_camera: Option<Vec3>,

    pub(crate) is_mmb_rotating: bool,
    pub(crate) mmb_anchor: Option<(f32, f32)>,

    // Game state
    pub(crate) selected_objects: Vec<ObjectId>,
    pub(crate) control_groups: HashMap<u8, Vec<ObjectId>>,
    /// Last control-group digit select (group, Instant) for double-tap camera jump residual.
    pub(crate) last_control_group_select: Option<(u8, Instant)>,
    /// Retail SAVE_VIEW1..8 / VIEW_VIEW1..8 camera bookmark residual (F1-F8).
    pub(crate) camera_view_bookmarks: [Option<Vec3>; 8],
    pub(crate) camera_rotate_left_held: bool,
    pub(crate) camera_rotate_right_held: bool,
    pub(crate) camera_zoom_in_held: bool,
    pub(crate) camera_zoom_out_held: bool,
    /// Retail TOGGLE_CAMERA_TRACKING_DRAWABLE residual.
    pub(crate) camera_tracking_selection: bool,
    /// Retail TOGGLE_FAST_FORWARD_REPLAY residual (TiVO fast mode).
    pub(crate) replay_fast_forward: bool,
    /// Retail DIPLOMACY KEY_TAB residual panel.
    pub(crate) diplomacy_panel: crate::ui::DiplomacyPanel,
    /// Retail CHAT_EVERYONE / CHAT_ALLIES residual panel.
    pub(crate) chat_panel: crate::ui::ChatPanel,
    pub(crate) current_player_id: u32,
    pub(crate) game_paused: bool,
    /// Main has adopted a live GameClient QuitMenu WND as the active offline
    /// pause owner.  This is distinct from the legacy Rust PauseMenu state so
    /// a real ButtonReturn can resume only the pause it created.
    pub(crate) quit_menu_host_active: bool,
    /// Main's one active script popup owns the current pause.  This is kept
    /// separate from `game_paused`: a popup acknowledgement must never resume
    /// a PauseMenu, QuitMenu, load, or result pause it did not create.
    pub(crate) popup_host_pause_owned: bool,

    // UI state
    pub(crate) show_debug_info: bool,
    pub(crate) show_health_bars: bool,
    /// FPS counter residual (options game.show_fps).
    pub(crate) show_fps: bool,
    /// Draw movement path lines residual.
    pub(crate) show_move_lines: bool,
    /// Draw attack-order lines residual.
    pub(crate) show_attack_lines: bool,
    pub(crate) frame_counter: u32,
    pub(crate) fps: f32,
    pub(crate) last_frame_timing: Option<FrameTiming>,
    pub(crate) frame_clock: FrameClock,
    pub(crate) menu_loading_tick_accumulator: Duration,
    pub(crate) menu_loading_last_tick: Instant,
    pub(crate) diagnostics_overlay: Option<DiagnosticsOverlayStats>,

    // UI system
    pub(crate) ui_manager: UIManager,
    pub(crate) game_hud: GameHUD,
    /// C++ structure placement template residual (awaiting map click).
    pub(crate) pending_structure_placement: Option<String>,
    /// C++ context command awaiting map click (AttackMove/Guard/SetRally residual).
    pub(crate) pending_map_command: Option<PendingMapCommand>,
    /// C++ `InGameUI::m_preventLeftClickDeselectionInAlternateMouseModeForOneClick`.
    pub(crate) prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: bool,

    pub(crate) active_menu_shell_hook: Option<&'static str>,
    pub(crate) runtime_host_headless: bool,
    /// True when `--runtime_host` is set (headless or windowed). Host cmds/status.
    pub(crate) runtime_host_active: bool,
    pub(crate) runtime_host_base_ui_screen: Option<String>,
    pub(crate) runtime_host_ui_screen_override: Option<String>,
    /// Sticky: open_skirmish_menu / Skirmish UI was entered this host session.
    pub(crate) runtime_host_saw_skirmish_menu: bool,
    pub(crate) runtime_host_last_gameplay_cmd: String,
    /// Main owns Rust snapshot persistence, while PopupSaveLoad owns the retail
    /// WND interaction.  This latches installation of the small typed bridge
    /// between the two so a normal mouse-driven popup never writes Common's
    /// separate GameState snapshot by accident.
    pub(crate) popup_save_load_bridge_initialized: bool,
    /// A runtime acceptance command chooses a deterministic slot/display name
    /// before it drives the real "New Save Game" confirmation.  The WND only
    /// supplies the description for that pseudo-row, so consume these once the
    /// confirmed callback reaches Main's actual save authority.
    pub(crate) pending_popup_save_slot: Option<String>,
    pub(crate) pending_popup_save_display_name: Option<String>,
    /// Real-person, windowed input evidence for the retail playable claim.
    /// Deliberately separate from `runtime_host_last_gameplay_cmd`.
    pub(crate) interactive_playability: InteractivePlayabilityEvidence,
    /// Parked UI match start waiting for the next Loading tick to call `load_map`.
    pub(crate) pending_match_start: Option<PendingMatchStart>,
    /// Carrier IDs admitted only from a successful physical right-click Gather
    /// command for the local player. `ReturningResources` drop-off events are
    /// matched against this set before they may latch physical evidence.
    pub(crate) physical_gather_carrier_ids: HashSet<ObjectId>,
    /// Cumulative HP damage applied this match (host_damage_log residual).
    pub(crate) match_damage_applied: f32,
    /// Cumulative destroy events from damage this match.
    pub(crate) match_kills: u32,
    /// Host asked for an immediate screenshot residual (bridge/event-loop consumes).
    pub(crate) runtime_host_pending_capture: bool,

    // Model loading state
    pub(crate) models_loaded: bool,
    pub(crate) pending_shell_model_prewarm: VecDeque<String>,
    pub(crate) menu_enter_frame: Option<u64>,
    pub(crate) shell_ui_enqueued_frame: Option<u64>,
    pub(crate) last_shell_prewarm_log: Option<Instant>,
    pub(crate) shell_prewarm_completion_logged: bool,
    /// How many Menu frames have rendered the full world scene so far.
    /// The first few Menu frames skip the world render to avoid a freeze while
    /// models/textures/terrain are loaded lazily for the first time.
    pub(crate) menu_world_frames_rendered: u32,
    pub(crate) last_slow_menu_tick_log: Option<Instant>,
    pub(crate) ingame_entered_at: Option<Instant>,
    pub(crate) match_over: bool,
    pub(crate) victory_summary: Option<VictorySummary>,
}

/// C++ SAGE engine VertexFormatXYZNDUV2 equivalent - matches original vertex declarations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],    // XYZ - Position coordinates
    pub normal: [f32; 3],      // N - Normal vector
    pub diffuse: u32,          // D - Diffuse color (RGBA packed as u32, like D3D8)
    pub tex_coords0: [f32; 2], // UV - Primary texture coordinates
    pub tex_coords1: [f32; 2], // UV2 - Secondary texture coordinates for multi-stage texturing
}

impl VertexXYZNDUV2 {
    /// C++ SAGE VertexFormatXYZNDUV2 buffer layout - matches D3DVERTEXELEMENT9 declarations
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexXYZNDUV2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position (XYZ)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal (N)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Diffuse color (D) - packed RGBA like D3D8
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Unorm8x4,
                },
                // Primary texture coordinates (UV)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<u32>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Secondary texture coordinates (UV2) for multi-texturing
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<u32>()
                        + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// C++ SAGE engine equivalent uniforms - matches GlobalUniforms structure
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SAGEUniforms {
    pub(crate) view_projection: [[f32; 4]; 4],
    pub(crate) view_matrix: [[f32; 4]; 4],
    pub(crate) projection_matrix: [[f32; 4]; 4],
    pub(crate) camera_position: [f32; 4],
    pub(crate) time: f32,
    pub(crate) ambient_light: [f32; 3],
    pub(crate) sun_direction: [f32; 3],
    pub(crate) sun_color: [f32; 3],
    pub(crate) _padding: f32,
}

/// C++ SAGE VertexMaterialClass equivalent - matches original material properties
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MaterialProperties {
    pub(crate) diffuse_color: [f32; 4], // Base color reflected by lighting
    pub(crate) specular_color: [f32; 4], // Sharp reflective highlights
    pub(crate) emissive_color: [f32; 4], // Self-illumination color
    pub(crate) opacity: f32,            // Transparency (1.0 = opaque, 0.0 = transparent)
    pub(crate) shininess: f32,          // Specular power
    pub(crate) stage0_uv_scale: [f32; 2], // UV scaling for stage 0
    pub(crate) stage1_uv_scale: [f32; 2], // UV scaling for stage 1
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StartupCameraDefaults {
    pub(crate) pitch_degrees: f32,
    pub(crate) yaw_degrees: f32,
    pub(crate) camera_height: f32,
    pub(crate) max_camera_height: f32,
}

#[cfg(feature = "game_client")]
pub(super) struct RegisteredGameClientBridge {
    pub(crate) client: crate::subsystem_manager::GameClientSubsystem,
    pub(crate) active: bool,
    pub(crate) state: SubsystemState,
}

#[cfg(feature = "game_client")]
impl RegisteredGameClientBridge {
    pub(super) fn new() -> SubsystemResult<Self> {
        Ok(Self {
            client: crate::subsystem_manager::GameClientSubsystem::new(),
            active: true,
            state: SubsystemState::Uninitialized,
        })
    }
}

#[cfg(feature = "game_client")]
impl GameClientInterface for RegisteredGameClientBridge {
    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;
        self.client
            .init()
            .map_err(|err| SubsystemError::InitializationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, delta_time: std::time::Duration) -> SubsystemResult<()> {
        self.client
            .update(delta_time.as_secs_f32())
            .map_err(|err| SubsystemError::UpdateFailed(err.to_string()))
    }

    fn render(&mut self) -> SubsystemResult<()> {
        // Rendering is owned by the Main runtime event loop.
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        self.client
            .reset()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;
        self.client
            .shutdown()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.active = false;
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn get_state(&self) -> SubsystemState {
        self.state
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

#[cfg(feature = "game_client")]
pub(super) fn register_command_list_bootstrap() {
    use game_client::message_stream::command_list::get_command_list;
    use game_engine::common::message_stream::SubsystemInterface;
    register_command_list_init(|| {
        if let Ok(mut cl) = get_command_list().write() {
            let _ = cl.init();
        }
    });
}

#[cfg(feature = "game_client")]
pub(super) fn register_real_game_client_bootstrap() {
    register_command_list_bootstrap();
}

#[cfg(not(feature = "game_client"))]
pub(super) fn register_real_game_client_bootstrap() {}

//! Windows message loop utilities (ported from WWLib msgloop.cpp/h).
//! Session state lives on [`crate::process_state::WwLibRuntime`].

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, IsDialogMessageW, MSG, PM_NOREMOVE, PeekMessageW,
    TranslateAcceleratorW, TranslateMessage, HACCEL,
};

#[cfg(target_os = "windows")]
pub fn windows_message_handler(runtime: &crate::process_state::WwLibRuntime) {
    let mut msg = MSG::default();
    while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_NOREMOVE).as_bool() {
        if !GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            return;
        }

        let mut processed = false;

        {
            let accels = runtime.accelerators.lock().unwrap();
            if let Some(&(window, accel)) = accels.first() {
                if window != 0
                    && TranslateAcceleratorW(HWND(window), HACCEL(accel), &msg).as_bool()
                {
                    processed = true;
                }
            }
        }
        if processed {
            continue;
        }

        {
            let dialogs = runtime.modeless_dialogs.lock().unwrap();
            for &dialog in dialogs.iter() {
                if IsDialogMessageW(HWND(dialog), &msg).as_bool() {
                    processed = true;
                    break;
                }
            }
        }
        if processed {
            continue;
        }

        {
            let handler_guard = runtime.message_intercept_handler.lock().unwrap();
            if let Some(handler) = *handler_guard {
                processed = handler(&mut msg);
            }
        }
        if processed {
            continue;
        }

        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn windows_message_handler(_runtime: &crate::process_state::WwLibRuntime) {}

#[cfg(target_os = "windows")]
pub fn add_modeless_dialog(runtime: &crate::process_state::WwLibRuntime, dialog: HWND) {
    runtime.modeless_dialogs.lock().unwrap().push(dialog.0);
}

#[cfg(target_os = "windows")]
pub fn remove_modeless_dialog(runtime: &crate::process_state::WwLibRuntime, dialog: HWND) {
    runtime
        .modeless_dialogs
        .lock()
        .unwrap()
        .retain(|&d| d != dialog.0);
}

#[cfg(target_os = "windows")]
pub fn add_accelerator(
    runtime: &crate::process_state::WwLibRuntime,
    window: HWND,
    accelerator: HACCEL,
) {
    runtime
        .accelerators
        .lock()
        .unwrap()
        .push((window.0, accelerator.0));
}

#[cfg(target_os = "windows")]
pub fn remove_accelerator(runtime: &crate::process_state::WwLibRuntime, accelerator: HACCEL) {
    runtime
        .accelerators
        .lock()
        .unwrap()
        .retain(|&(_, a)| a != accelerator.0);
}

#[cfg(not(target_os = "windows"))]
pub fn add_modeless_dialog(_runtime: &crate::process_state::WwLibRuntime, _dialog: usize) {}

#[cfg(not(target_os = "windows"))]
pub fn remove_modeless_dialog(_runtime: &crate::process_state::WwLibRuntime, _dialog: usize) {}

#[cfg(not(target_os = "windows"))]
pub fn add_accelerator(
    _runtime: &crate::process_state::WwLibRuntime,
    _window: usize,
    _accelerator: usize,
) {
}

#[cfg(not(target_os = "windows"))]
pub fn remove_accelerator(_runtime: &crate::process_state::WwLibRuntime, _accelerator: usize) {}

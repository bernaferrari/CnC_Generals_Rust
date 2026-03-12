//! Windows message loop utilities (ported from WWLib msgloop.cpp/h).

use crate::vector_class::DynamicVectorClass;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, IsDialogMessageW, PeekMessageW, TranslateAcceleratorW,
    TranslateMessage, MSG, PM_NOREMOVE,
};

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct AcceleratorTracker {
    window: HWND,
    accelerator: windows::Win32::UI::WindowsAndMessaging::HACCEL,
}

#[cfg(target_os = "windows")]
impl Default for AcceleratorTracker {
    fn default() -> Self {
        AcceleratorTracker {
            window: HWND(0),
            accelerator: windows::Win32::UI::WindowsAndMessaging::HACCEL(0),
        }
    }
}

#[cfg(target_os = "windows")]
impl PartialEq for AcceleratorTracker {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window && self.accelerator == other.accelerator
    }
}

#[cfg(target_os = "windows")]
static mut MODELESS_DIALOGS: Option<DynamicVectorClass<HWND>> = None;

#[cfg(target_os = "windows")]
static mut ACCELERATORS: Option<DynamicVectorClass<AcceleratorTracker>> = None;

#[cfg(target_os = "windows")]
pub static mut MESSAGE_INTERCEPT_HANDLER: Option<fn(&mut MSG) -> bool> = None;

#[cfg(target_os = "windows")]
fn modeless_dialogs() -> &'static mut DynamicVectorClass<HWND> {
    unsafe {
        if MODELESS_DIALOGS.is_none() {
            MODELESS_DIALOGS = Some(DynamicVectorClass::new(0, None));
        }
        MODELESS_DIALOGS.as_mut().unwrap()
    }
}

#[cfg(target_os = "windows")]
fn accelerators() -> &'static mut DynamicVectorClass<AcceleratorTracker> {
    unsafe {
        if ACCELERATORS.is_none() {
            ACCELERATORS = Some(DynamicVectorClass::new(0, None));
        }
        ACCELERATORS.as_mut().unwrap()
    }
}

#[cfg(target_os = "windows")]
pub fn windows_message_handler() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_NOREMOVE).as_bool() {
            if !GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                return;
            }

            let mut processed = false;

            let accels = accelerators();
            for aindex in 0..accels.count() {
                if accels[aindex].window != HWND(0) {
                    if TranslateAcceleratorW(
                        accels[aindex].window,
                        accels[aindex].accelerator,
                        &msg,
                    )
                    .as_bool()
                    {
                        processed = true;
                    }
                }
                break;
            }
            if processed {
                continue;
            }

            let dialogs = modeless_dialogs();
            for index in 0..dialogs.count() {
                if IsDialogMessageW(dialogs[index], &msg).as_bool() {
                    processed = true;
                    break;
                }
            }
            if processed {
                continue;
            }

            if let Some(handler) = MESSAGE_INTERCEPT_HANDLER {
                processed = handler(&mut msg);
            }
            if processed {
                continue;
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn windows_message_handler() {}

#[cfg(target_os = "windows")]
pub fn add_modeless_dialog(dialog: HWND) {
    modeless_dialogs().add(dialog);
}

#[cfg(target_os = "windows")]
pub fn remove_modeless_dialog(dialog: HWND) {
    modeless_dialogs().delete_by_value(&dialog);
}

#[cfg(target_os = "windows")]
pub fn add_accelerator(window: HWND, accelerator: windows::Win32::UI::WindowsAndMessaging::HACCEL) {
    accelerators().add(AcceleratorTracker {
        window,
        accelerator,
    });
}

#[cfg(target_os = "windows")]
pub fn remove_accelerator(accelerator: windows::Win32::UI::WindowsAndMessaging::HACCEL) {
    let accels = accelerators();
    for index in 0..accels.count() {
        if accels[index].accelerator == accelerator {
            accels.delete(index);
            break;
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn add_modeless_dialog(_dialog: usize) {}

#[cfg(not(target_os = "windows"))]
pub fn remove_modeless_dialog(_dialog: usize) {}

#[cfg(not(target_os = "windows"))]
pub fn add_accelerator(_window: usize, _accelerator: usize) {}

#[cfg(not(target_os = "windows"))]
pub fn remove_accelerator(_accelerator: usize) {}

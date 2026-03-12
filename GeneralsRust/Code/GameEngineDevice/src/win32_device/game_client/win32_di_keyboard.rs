//! Win32 DirectInput Keyboard Implementation
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/Win32Device/GameClient/Win32DIKeyboard.cpp
//! 
//! This module provides complete DirectInput keyboard functionality with modern Rust patterns
//! while maintaining exact compatibility with the C++ implementation.

use std::{
    collections::VecDeque,
    ffi::c_void,
    ptr,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
};

use anyhow::{Result, Context};
use thiserror::Error;
use tracing::{debug, error, warn, info};

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::LibraryLoader::*,
    Win32::UI::Input::DirectInput::*,
    Win32::UI::WindowsAndMessaging::*,
};

/// Constants for DirectInput keyboard
const KEYBOARD_BUFFER_SIZE: usize = 256;
const DIRECTINPUT_VERSION: u32 = 0x0800;

/// Keyboard state constants (matching C++ KeyDefs.h)
pub const KEY_STATE_UP: u16 = 0x00;
pub const KEY_STATE_DOWN: u16 = 0x01;
pub const KEY_STATE_CAPSLOCK: u16 = 0x02;

/// Key constants (matching DirectInput scancodes)
pub const KEY_NONE: u8 = 0xFF;
pub const KEY_LOST: u8 = 0xFE;

/// Keyboard input/output structure (matching C++ KeyboardIO)
#[derive(Debug, Clone, Copy)]
pub struct KeyboardIO {
    /// Key scancode
    pub key: u8,
    /// Status flags  
    pub status: u8,
    /// Key state (up/down/caps)
    pub state: u16,
    /// Sequence number from DirectInput
    pub sequence: u32,
}

impl Default for KeyboardIO {
    fn default() -> Self {
        Self {
            key: KEY_NONE,
            status: 0,
            state: KEY_STATE_UP,
            sequence: 0,
        }
    }
}

/// Status type for KeyboardIO
pub mod keyboard_status {
    pub const STATUS_UNUSED: u8 = 0x00;
    pub const STATUS_USED: u8 = 0x01;
}

/// DirectInput keyboard errors
#[derive(Error, Debug)]
pub enum DirectInputKeyboardError {
    #[error("Failed to initialize DirectInput: {0}")]
    InitializationFailed(String),
    #[error("Device not acquired")]
    DeviceNotAcquired,
    #[error("Input lost")]
    InputLost,
    #[error("DirectInput error: {code:08x}")]
    DirectInputError { code: u32 },
    #[error("Windows error: {0}")]
    WindowsError(#[from] windows::core::Error),
}

/// DirectInput keyboard implementation
pub struct DirectInputKeyboard {
    #[cfg(windows)]
    direct_input: Option<IDirectInput8A>,
    #[cfg(windows)]
    keyboard_device: Option<IDirectInputDevice8A>,
    #[cfg(not(windows))]
    _phantom: std::marker::PhantomData<()>,
    
    /// Application instance handle
    app_instance: HINSTANCE,
    /// Application window handle
    app_window: HWND,
    /// Modifier state tracking
    modifiers: u16,
    /// Error count tracking
    error_count: usize,
    /// Device initialized flag
    initialized: AtomicBool,
}

impl DirectInputKeyboard {
    /// Create a new DirectInput keyboard instance
    pub fn new(app_instance: HINSTANCE, app_window: HWND) -> Self {
        Self {
            #[cfg(windows)]
            direct_input: None,
            #[cfg(windows)]
            keyboard_device: None,
            #[cfg(not(windows))]
            _phantom: std::marker::PhantomData,
            app_instance,
            app_window,
            modifiers: 0,
            error_count: 0,
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the DirectInput keyboard system
    pub fn init(&mut self) -> Result<(), DirectInputKeyboardError> {
        #[cfg(windows)]
        {
            self.open_keyboard()?;
            self.update_caps_lock_state();
            self.initialized.store(true, Ordering::SeqCst);
            info!("DirectInput keyboard initialized successfully");
        }
        #[cfg(not(windows))]
        {
            warn!("DirectInput keyboard not supported on this platform");
            self.initialized.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Reset the keyboard system
    pub fn reset(&mut self) -> Result<(), DirectInputKeyboardError> {
        debug!("Resetting DirectInput keyboard");
        self.error_count = 0;
        self.modifiers = 0;
        self.update_caps_lock_state();
        Ok(())
    }

    /// Update keyboard state (called once per frame)
    pub fn update(&mut self) -> Result<(), DirectInputKeyboardError> {
        // Update caps lock state
        self.update_caps_lock_state();
        Ok(())
    }

    /// Get a single keyboard event
    pub fn get_key(&mut self) -> Result<KeyboardIO, DirectInputKeyboardError> {
        #[cfg(windows)]
        {
            self.get_key_internal()
        }
        #[cfg(not(windows))]
        {
            Ok(KeyboardIO::default())
        }
    }

    /// Check if caps lock is active
    pub fn get_caps_state(&self) -> bool {
        #[cfg(windows)]
        unsafe {
            let caps_state = GetKeyState(VK_CAPITAL.0 as i32);
            (caps_state & 0x01) != 0
        }
        #[cfg(not(windows))]
        false
    }

    /// Close and cleanup the keyboard system
    pub fn close(&mut self) {
        #[cfg(windows)]
        {
            self.close_keyboard();
        }
        self.initialized.store(false, Ordering::SeqCst);
        debug!("DirectInput keyboard closed");
    }

    /// Check if the keyboard is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    #[cfg(windows)]
    fn open_keyboard(&mut self) -> Result<(), DirectInputKeyboardError> {
        use windows::Win32::System::Com::*;
        
        // Create DirectInput interface
        let mut direct_input: Option<IDirectInput8A> = None;
        unsafe {
            DirectInput8Create(
                self.app_instance,
                DIRECTINPUT_VERSION,
                &IDirectInput8A::IID,
                &mut direct_input as *mut _ as *mut *mut c_void,
                None,
            ).map_err(|e| {
                error!("Failed to create DirectInput8: {}", e);
                DirectInputKeyboardError::InitializationFailed(format!("DirectInput8Create failed: {}", e))
            })?;
        }

        let direct_input = direct_input
            .ok_or_else(|| DirectInputKeyboardError::InitializationFailed("DirectInput8 is null".to_string()))?;

        // Create keyboard device
        let mut keyboard_device: Option<IDirectInputDevice8A> = None;
        unsafe {
            direct_input
                .CreateDevice(&GUID_SysKeyboard, &mut keyboard_device, None)
                .map_err(|e| {
                    error!("Failed to create keyboard device: {}", e);
                    DirectInputKeyboardError::InitializationFailed(format!("CreateDevice failed: {}", e))
                })?;
        }

        let keyboard_device = keyboard_device
            .ok_or_else(|| DirectInputKeyboardError::InitializationFailed("Keyboard device is null".to_string()))?;

        // Set data format
        unsafe {
            keyboard_device
                .SetDataFormat(&c_dfDIKeyboard)
                .map_err(|e| {
                    error!("Failed to set keyboard data format: {}", e);
                    DirectInputKeyboardError::InitializationFailed(format!("SetDataFormat failed: {}", e))
                })?;
        }

        // Set cooperative level - non-exclusive for NT compatibility
        unsafe {
            keyboard_device
                .SetCooperativeLevel(
                    self.app_window,
                    DISCL_FOREGROUND | DISCL_NONEXCLUSIVE,
                )
                .map_err(|e| {
                    error!("Failed to set keyboard cooperative level: {}", e);
                    DirectInputKeyboardError::InitializationFailed(format!("SetCooperativeLevel failed: {}", e))
                })?;
        }

        // Set buffer size
        let mut prop: DIPROPDWORD = unsafe { std::mem::zeroed() };
        prop.diph.dwSize = std::mem::size_of::<DIPROPDWORD>() as u32;
        prop.diph.dwHeaderSize = std::mem::size_of::<DIPROPHEADER>() as u32;
        prop.diph.dwObj = 0;
        prop.diph.dwHow = DIPH_DEVICE;
        prop.dwData = KEYBOARD_BUFFER_SIZE as u32;

        unsafe {
            keyboard_device
                .SetProperty(&DIPROP_BUFFERSIZE, &prop.diph)
                .map_err(|e| {
                    error!("Failed to set keyboard buffer size: {}", e);
                    DirectInputKeyboardError::InitializationFailed(format!("SetProperty failed: {}", e))
                })?;
        }

        // Acquire the keyboard
        unsafe {
            match keyboard_device.Acquire() {
                Ok(_) => info!("Keyboard acquired successfully"),
                Err(e) => {
                    warn!("Failed to acquire keyboard (may be in windowed mode): {}", e);
                    // Don't fail here as we can re-acquire later
                }
            }
        }

        self.direct_input = Some(direct_input);
        self.keyboard_device = Some(keyboard_device);

        debug!("DirectInput keyboard opened successfully");
        Ok(())
    }

    #[cfg(windows)]
    fn close_keyboard(&mut self) {
        if let Some(ref keyboard_device) = self.keyboard_device {
            unsafe {
                let _ = keyboard_device.Unacquire();
            }
            debug!("Keyboard device unacquired");
        }
        
        self.keyboard_device = None;
        self.direct_input = None;
        debug!("DirectInput keyboard closed");
    }

    #[cfg(windows)]
    fn get_key_internal(&mut self) -> Result<KeyboardIO, DirectInputKeyboardError> {
        let keyboard_device = self.keyboard_device
            .as_ref()
            .ok_or(DirectInputKeyboardError::DeviceNotAcquired)?;

        let mut key_data: DIDEVICEOBJECTDATA = unsafe { std::mem::zeroed() };
        let mut num_items = 1u32;

        // First try to acquire the device
        unsafe {
            let acquire_result = keyboard_device.Acquire();
            match acquire_result {
                Ok(_) | Err(windows::core::Error { code: windows::core::HRESULT(0x80040001), .. }) => {
                    // DI_OK or S_FALSE (already acquired)
                }
                Err(e) => {
                    debug!("Could not acquire keyboard: {}", e);
                    return Ok(KeyboardIO::default());
                }
            }

            // Get device data
            match keyboard_device.GetDeviceData(
                std::mem::size_of::<DIDEVICEOBJECTDATA>() as u32,
                &mut key_data,
                &mut num_items,
                0,
            ) {
                Ok(_) => {
                    // Success case
                    if num_items == 0 {
                        return Ok(KeyboardIO::default());
                    }

                    let mut result = KeyboardIO {
                        key: (key_data.dwOfs & 0xFF) as u8,
                        sequence: key_data.dwSequence,
                        state: if (key_data.dwData & 0x80) != 0 {
                            KEY_STATE_DOWN
                        } else {
                            KEY_STATE_UP
                        },
                        status: keyboard_status::STATUS_UNUSED,
                    };

                    debug!("Key event: key={:02x}, state={}, seq={}", 
                           result.key, result.state, result.sequence);
                    Ok(result)
                }
                Err(e) => {
                    let hresult_code = e.code().0 as u32;
                    match hresult_code {
                        0x8007001E => {
                            // DIERR_INPUTLOST
                            self.handle_input_lost()?;
                            Ok(KeyboardIO {
                                key: KEY_LOST,
                                ..Default::default()
                            })
                        }
                        0x80040001 => {
                            // DIERR_NOTACQUIRED
                            self.handle_not_acquired()?;
                            Ok(KeyboardIO {
                                key: KEY_LOST,
                                ..Default::default()
                            })
                        }
                        _ => {
                            warn!("DirectInput keyboard error: {:08x}", hresult_code);
                            self.error_count += 1;
                            Ok(KeyboardIO::default())
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    fn handle_input_lost(&mut self) -> Result<(), DirectInputKeyboardError> {
        debug!("Handling keyboard input lost");
        if let Some(ref keyboard_device) = self.keyboard_device {
            unsafe {
                match keyboard_device.Acquire() {
                    Ok(_) => {
                        debug!("Successfully re-acquired keyboard after input lost");
                        Ok(())
                    }
                    Err(e) => {
                        debug!("Failed to re-acquire keyboard: {}", e);
                        Err(DirectInputKeyboardError::InputLost)
                    }
                }
            }
        } else {
            Err(DirectInputKeyboardError::DeviceNotAcquired)
        }
    }

    #[cfg(windows)]
    fn handle_not_acquired(&mut self) -> Result<(), DirectInputKeyboardError> {
        debug!("Handling keyboard not acquired");
        if let Some(ref keyboard_device) = self.keyboard_device {
            unsafe {
                match keyboard_device.Acquire() {
                    Ok(_) => {
                        debug!("Successfully acquired keyboard");
                        Ok(())
                    }
                    Err(e) => {
                        debug!("Failed to acquire keyboard: {}", e);
                        Err(DirectInputKeyboardError::DeviceNotAcquired)
                    }
                }
            }
        } else {
            Err(DirectInputKeyboardError::DeviceNotAcquired)
        }
    }

    fn update_caps_lock_state(&mut self) {
        #[cfg(windows)]
        unsafe {
            let caps_state = GetKeyState(VK_CAPITAL.0 as i32);
            if (caps_state & 0x01) != 0 {
                self.modifiers |= KEY_STATE_CAPSLOCK;
            } else {
                self.modifiers &= !KEY_STATE_CAPSLOCK;
            }
        }
    }

    /// Get current modifier state
    pub fn get_modifiers(&self) -> u16 {
        self.modifiers
    }

    /// Get error count
    pub fn get_error_count(&self) -> usize {
        self.error_count
    }
}

impl Drop for DirectInputKeyboard {
    fn drop(&mut self) {
        self.close();
    }
}

// Thread safety - DirectInput keyboard is designed to be used from the main thread only
unsafe impl Send for DirectInputKeyboard {}

/// Utility function to print DirectInput error codes for debugging
#[cfg(windows)]
pub fn print_directinput_error(label: &str, hr: windows::core::HRESULT) {
    let error_name = match hr.0 as u32 {
        0x80040001 => "DIERR_ACQUIRED",
        0x80040002 => "DIERR_ALREADYINITIALIZED", 
        0x80040003 => "DIERR_BADDRIVERVER",
        0x80040004 => "DIERR_BETADIRECTINPUTVERSION",
        0x80040005 => "DIERR_DEVICEFULL",
        0x80040006 => "DIERR_DEVICENOTREG",
        0x80040007 => "DIERR_EFFECTPLAYING",
        0x80040008 => "DIERR_GENERIC",
        0x80040009 => "DIERR_HANDLEEXISTS",
        0x8004000A => "DIERR_HASEFFECTS",
        0x8004000B => "DIERR_INCOMPLETEEFFECT",
        0x8007001E => "DIERR_INPUTLOST",
        0x80070057 => "DIERR_INVALIDPARAM",
        0x8004000C => "DIERR_MAPFILEFAIL",
        0x8004000D => "DIERR_MOREDATA",
        0x8004000E => "DIERR_NOAGGREGATION", 
        0x8004000F => "DIERR_NOINTERFACE",
        0x80040010 => "DIERR_NOTACQUIRED",
        0x80040011 => "DIERR_NOTBUFFERED",
        0x80040012 => "DIERR_NOTDOWNLOADED",
        0x80040013 => "DIERR_NOTEXCLUSIVEACQUIRED",
        0x80040014 => "DIERR_NOTFOUND",
        0x80040015 => "DIERR_NOTINITIALIZED",
        0x80040016 => "DIERR_OBJECTNOTFOUND",
        0x80040017 => "DIERR_OLDDIRECTINPUTVERSION",
        0x80040018 => "DIERR_OTHERAPPHASPRIO",
        0x8007000E => "DIERR_OUTOFMEMORY",
        0x80040019 => "DIERR_READONLY",
        0x8004001A => "DIERR_REPORTFULL",
        0x8004001B => "DIERR_UNPLUGGED",
        0x8004001C => "DIERR_UNSUPPORTED",
        _ => "UNKNOWN_ERROR",
    };
    
    debug!("{}: '{}' - '0x{:08x}'", label, error_name, hr.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_io_default() {
        let io = KeyboardIO::default();
        assert_eq!(io.key, KEY_NONE);
        assert_eq!(io.status, 0);
        assert_eq!(io.state, KEY_STATE_UP);
        assert_eq!(io.sequence, 0);
    }

    #[test] 
    fn test_keyboard_constants() {
        assert_eq!(KEY_STATE_UP, 0x00);
        assert_eq!(KEY_STATE_DOWN, 0x01);
        assert_eq!(KEY_STATE_CAPSLOCK, 0x02);
    }

    #[test]
    fn test_keyboard_creation() {
        #[cfg(windows)]
        {
            let keyboard = DirectInputKeyboard::new(HINSTANCE(0), HWND(0));
            assert!(!keyboard.is_initialized());
            assert_eq!(keyboard.get_error_count(), 0);
        }
        #[cfg(not(windows))]
        {
            let keyboard = DirectInputKeyboard::new(0 as _, 0 as _);
            assert!(!keyboard.is_initialized());
            assert_eq!(keyboard.get_error_count(), 0);
        }
    }
}
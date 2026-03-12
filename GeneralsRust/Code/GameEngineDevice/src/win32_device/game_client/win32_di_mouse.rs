//! Win32 DirectInput Mouse Implementation
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/Win32Device/GameClient/Win32DIMouse.cpp
//! 
//! This module provides complete DirectInput mouse functionality with modern Rust patterns
//! while maintaining exact compatibility with the C++ implementation.

use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, error, warn, info};

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::*,
    Win32::UI::Input::DirectInput::*,
    Win32::UI::WindowsAndMessaging::*,
};

/// Constants for DirectInput mouse
const MOUSE_BUFFER_SIZE: usize = 256;
const DIRECTINPUT_VERSION: u32 = 0x0800;

/// Mouse event result constants
pub const MOUSE_NONE: u8 = 0x00;
pub const MOUSE_OK: u8 = 0x01;
pub const MOUSE_FAILED: u8 = 0x80;
pub const MOUSE_LOST: u8 = 0xFF;

/// Mouse button state constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseButtonState {
    Up = 0,
    Down = 1,
    DoubleClick = 2,
}

/// Mouse cursor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MouseCursor {
    None = 0,
    Normal = 1,
    Arrow = 2,
    Scroll = 3,
    Cross = 4,
}

/// Point structure for mouse coordinates
#[derive(Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Mouse input/output structure (matching C++ MouseIO)
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseIO {
    /// Left button state
    pub left_state: MouseButtonState,
    /// Middle button state  
    pub middle_state: MouseButtonState,
    /// Right button state
    pub right_state: MouseButtonState,
    /// Frame when left button was pressed
    pub left_frame: u32,
    /// Frame when middle button was pressed
    pub middle_frame: u32,
    /// Frame when right button was pressed
    pub right_frame: u32,
    /// Mouse position
    pub pos: Point,
    /// Mouse wheel position (delta)
    pub wheel_pos: i32,
    /// Event timestamp
    pub time: u32,
}

impl Default for MouseButtonState {
    fn default() -> Self {
        MouseButtonState::Up
    }
}

/// DirectInput mouse errors
#[derive(Error, Debug)]
pub enum DirectInputMouseError {
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

/// DirectInput mouse implementation
pub struct DirectInputMouse {
    #[cfg(windows)]
    direct_input: Option<IDirectInput8A>,
    #[cfg(windows)]
    mouse_device: Option<IDirectInputDevice8A>,
    #[cfg(not(windows))]
    _phantom: std::marker::PhantomData<()>,
    
    /// Application instance handle
    app_instance: HINSTANCE,
    /// Application window handle
    app_window: HWND,
    /// Number of mouse buttons
    num_buttons: u8,
    /// Number of axes
    num_axes: u8,
    /// Force feedback capability
    force_feedback: bool,
    /// Current mouse cursor
    current_cursor: MouseCursor,
    /// Device initialized flag
    initialized: AtomicBool,
    /// Current mouse position
    current_position: Point,
    /// Error count tracking
    error_count: usize,
}

impl DirectInputMouse {
    /// Create a new DirectInput mouse instance
    pub fn new(app_instance: HINSTANCE, app_window: HWND) -> Self {
        Self {
            #[cfg(windows)]
            direct_input: None,
            #[cfg(windows)]
            mouse_device: None,
            #[cfg(not(windows))]
            _phantom: std::marker::PhantomData,
            app_instance,
            app_window,
            num_buttons: 3,
            num_axes: 3,
            force_feedback: false,
            current_cursor: MouseCursor::Normal,
            initialized: AtomicBool::new(false),
            current_position: Point::default(),
            error_count: 0,
        }
    }

    /// Initialize the DirectInput mouse system
    pub fn init(&mut self) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        {
            self.open_mouse()?;
            self.update_cursor_position();
            self.initialized.store(true, Ordering::SeqCst);
            info!("DirectInput mouse initialized successfully");
        }
        #[cfg(not(windows))]
        {
            warn!("DirectInput mouse not supported on this platform");
            self.initialized.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Reset the mouse system
    pub fn reset(&mut self) -> Result<(), DirectInputMouseError> {
        debug!("Resetting DirectInput mouse");
        self.error_count = 0;
        self.current_position = Point::default();
        Ok(())
    }

    /// Update mouse state (called once per frame)
    pub fn update(&mut self) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        {
            // Update cursor position from Windows cursor
            self.update_cursor_position();
        }
        Ok(())
    }

    /// Get a mouse event from the device
    pub fn get_mouse_event(&mut self, flush: bool) -> Result<(u8, MouseIO), DirectInputMouseError> {
        #[cfg(windows)]
        {
            self.get_mouse_event_internal(flush)
        }
        #[cfg(not(windows))]
        {
            Ok((MOUSE_NONE, MouseIO::default()))
        }
    }

    /// Set mouse cursor
    pub fn set_cursor(&mut self, cursor: MouseCursor) -> Result<(), DirectInputMouseError> {
        if self.current_cursor == cursor {
            return Ok(());
        }

        #[cfg(windows)]
        unsafe {
            let win32_cursor = match cursor {
                MouseCursor::None => None,
                MouseCursor::Normal | MouseCursor::Arrow => {
                    Some(LoadCursorW(None, IDC_ARROW).unwrap())
                }
                MouseCursor::Scroll => {
                    Some(LoadCursorW(None, IDC_SIZEALL).unwrap())
                }
                MouseCursor::Cross => {
                    Some(LoadCursorW(None, IDC_CROSS).unwrap())
                }
            };

            if let Some(cursor_handle) = win32_cursor {
                SetCursor(cursor_handle);
            } else {
                SetCursor(HCURSOR(0));
            }
        }

        self.current_cursor = cursor;
        debug!("Mouse cursor set to {:?}", cursor);
        Ok(())
    }

    /// Set mouse position
    pub fn set_position(&mut self, x: i32, y: i32) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        unsafe {
            let mut point = POINT { x, y };
            ClientToScreen(self.app_window, &mut point);
            SetCursorPos(point.x, point.y);
        }
        
        self.current_position = Point { x, y };
        debug!("Mouse position set to ({}, {})", x, y);
        Ok(())
    }

    /// Capture the mouse
    pub fn capture(&self) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        unsafe {
            SetCapture(self.app_window);
        }
        debug!("Mouse captured");
        Ok(())
    }

    /// Release mouse capture
    pub fn release_capture(&self) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        unsafe {
            ReleaseCapture();
        }
        debug!("Mouse capture released");
        Ok(())
    }

    /// Set mouse movement limits
    pub fn set_mouse_limits(&self, windowed: bool) -> Result<(), DirectInputMouseError> {
        #[cfg(windows)]
        if windowed {
            unsafe {
                let mut window_rect = RECT::default();
                GetWindowRect(self.app_window, &mut window_rect);
                ClipCursor(Some(&window_rect));
                debug!("Mouse clipped to window bounds");
            }
        } else {
            unsafe {
                ClipCursor(None);
                debug!("Mouse clipping removed");
            }
        }
        Ok(())
    }

    /// Close and cleanup the mouse system
    pub fn close(&mut self) {
        #[cfg(windows)]
        {
            self.close_mouse();
        }
        self.initialized.store(false, Ordering::SeqCst);
        debug!("DirectInput mouse closed");
    }

    /// Check if the mouse is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Get mouse capabilities
    pub fn get_capabilities(&self) -> (u8, u8, bool) {
        (self.num_buttons, self.num_axes, self.force_feedback)
    }

    /// Get current mouse position
    pub fn get_position(&self) -> Point {
        self.current_position
    }

    /// Get current cursor type
    pub fn get_cursor(&self) -> MouseCursor {
        self.current_cursor
    }

    /// Get error count
    pub fn get_error_count(&self) -> usize {
        self.error_count
    }

    #[cfg(windows)]
    fn open_mouse(&mut self) -> Result<(), DirectInputMouseError> {
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
                DirectInputMouseError::InitializationFailed(format!("DirectInput8Create failed: {}", e))
            })?;
        }

        let direct_input = direct_input
            .ok_or_else(|| DirectInputMouseError::InitializationFailed("DirectInput8 is null".to_string()))?;

        // Create mouse device
        let mut mouse_device: Option<IDirectInputDevice8A> = None;
        unsafe {
            direct_input
                .CreateDevice(&GUID_SysMouse, &mut mouse_device, None)
                .map_err(|e| {
                    error!("Failed to create mouse device: {}", e);
                    DirectInputMouseError::InitializationFailed(format!("CreateDevice failed: {}", e))
                })?;
        }

        let mouse_device = mouse_device
            .ok_or_else(|| DirectInputMouseError::InitializationFailed("Mouse device is null".to_string()))?;

        // Set data format
        unsafe {
            mouse_device
                .SetDataFormat(&c_dfDIMouse)
                .map_err(|e| {
                    error!("Failed to set mouse data format: {}", e);
                    DirectInputMouseError::InitializationFailed(format!("SetDataFormat failed: {}", e))
                })?;
        }

        // Set cooperative level
        unsafe {
            mouse_device
                .SetCooperativeLevel(
                    self.app_window,
                    DISCL_NONEXCLUSIVE | DISCL_FOREGROUND,
                )
                .map_err(|e| {
                    error!("Failed to set mouse cooperative level: {}", e);
                    DirectInputMouseError::InitializationFailed(format!("SetCooperativeLevel failed: {}", e))
                })?;
        }

        // Set buffer size
        let mut prop: DIPROPDWORD = unsafe { std::mem::zeroed() };
        prop.diph.dwSize = std::mem::size_of::<DIPROPDWORD>() as u32;
        prop.diph.dwHeaderSize = std::mem::size_of::<DIPROPHEADER>() as u32;
        prop.diph.dwObj = 0;
        prop.diph.dwHow = DIPH_DEVICE;
        prop.dwData = MOUSE_BUFFER_SIZE as u32;

        unsafe {
            mouse_device
                .SetProperty(&DIPROP_BUFFERSIZE, &prop.diph)
                .map_err(|e| {
                    error!("Failed to set mouse buffer size: {}", e);
                    DirectInputMouseError::InitializationFailed(format!("SetProperty failed: {}", e))
                })?;
        }

        // Acquire the mouse
        unsafe {
            match mouse_device.Acquire() {
                Ok(_) => info!("Mouse acquired successfully"),
                Err(e) => {
                    error!("Failed to acquire mouse: {}", e);
                    return Err(DirectInputMouseError::InitializationFailed(format!("Acquire failed: {}", e)));
                }
            }
        }

        // Get device capabilities
        let mut caps: DIDEVCAPS = unsafe { std::mem::zeroed() };
        caps.dwSize = std::mem::size_of::<DIDEVCAPS>() as u32;
        
        unsafe {
            match mouse_device.GetCapabilities(&mut caps) {
                Ok(_) => {
                    self.num_buttons = caps.dwButtons as u8;
                    self.num_axes = caps.dwAxes as u8;
                    self.force_feedback = (caps.dwFlags & DIDC_FORCEFEEDBACK.0) != 0;
                    info!("Mouse info: Buttons = {}, Force Feedback = {}, Axes = {}", 
                          self.num_buttons, 
                          if self.force_feedback { "Yes" } else { "No" }, 
                          self.num_axes);
                }
                Err(e) => {
                    warn!("Failed to get mouse capabilities: {}", e);
                    // Use defaults
                }
            }
        }

        self.direct_input = Some(direct_input);
        self.mouse_device = Some(mouse_device);

        debug!("DirectInput mouse opened successfully");
        Ok(())
    }

    #[cfg(windows)]
    fn close_mouse(&mut self) {
        if let Some(ref mouse_device) = self.mouse_device {
            unsafe {
                let _ = mouse_device.Unacquire();
            }
            debug!("Mouse device unacquired");
        }
        
        self.mouse_device = None;
        self.direct_input = None;
        debug!("DirectInput mouse closed");
    }

    #[cfg(windows)]
    fn get_mouse_event_internal(&mut self, _flush: bool) -> Result<(u8, MouseIO), DirectInputMouseError> {
        let mouse_device = self.mouse_device
            .as_ref()
            .ok_or(DirectInputMouseError::DeviceNotAcquired)?;

        let mut mouse_data: DIDEVICEOBJECTDATA = unsafe { std::mem::zeroed() };
        let mut num_items = 1u32;

        // Poll and get device data
        unsafe {
            let _ = mouse_device.Poll();
            
            match mouse_device.GetDeviceData(
                std::mem::size_of::<DIDEVICEOBJECTDATA>() as u32,
                &mut mouse_data,
                &mut num_items,
                0,
            ) {
                Ok(_) => {
                    if num_items == 0 {
                        return Ok((MOUSE_NONE, MouseIO::default()));
                    }

                    let mut result = MouseIO::default();
                    self.map_directinput_mouse(&mut result, &mouse_data);
                    debug!("Mouse event: offset={}, data={}", mouse_data.dwOfs, mouse_data.dwData);
                    Ok((MOUSE_OK, result))
                }
                Err(e) => {
                    let hresult_code = e.code().0 as u32;
                    match hresult_code {
                        0x8007001E => {
                            // DIERR_INPUTLOST
                            self.handle_input_lost()?;
                            Ok((MOUSE_LOST, MouseIO::default()))
                        }
                        0x80040010 => {
                            // DIERR_NOTACQUIRED
                            self.handle_not_acquired()?;
                            Ok((MOUSE_LOST, MouseIO::default()))
                        }
                        _ => {
                            warn!("DirectInput mouse error: {:08x}", hresult_code);
                            self.error_count += 1;
                            Ok((MOUSE_NONE, MouseIO::default()))
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    fn map_directinput_mouse(&self, mouse: &mut MouseIO, data: &DIDEVICEOBJECTDATA) {
        match data.dwOfs {
            DIMOFS_BUTTON0 => {
                mouse.left_state = if (data.dwData & 0x80) != 0 {
                    MouseButtonState::Down
                } else {
                    MouseButtonState::Up
                };
                mouse.left_frame = data.dwSequence;
            }
            DIMOFS_BUTTON1 => {
                mouse.right_state = if (data.dwData & 0x80) != 0 {
                    MouseButtonState::Down
                } else {
                    MouseButtonState::Up
                };
                mouse.right_frame = data.dwSequence;
            }
            DIMOFS_BUTTON2 => {
                mouse.middle_state = if (data.dwData & 0x80) != 0 {
                    MouseButtonState::Down
                } else {
                    MouseButtonState::Up
                };
                mouse.middle_frame = data.dwSequence;
            }
            DIMOFS_X => {
                mouse.pos.x = data.dwData as i32;
            }
            DIMOFS_Y => {
                mouse.pos.y = data.dwData as i32;
            }
            DIMOFS_Z => {
                mouse.wheel_pos = data.dwData as i32;
            }
            _ => {
                // Additional buttons or other data
                debug!("Unhandled mouse data offset: {}", data.dwOfs);
            }
        }
    }

    #[cfg(windows)]
    fn handle_input_lost(&mut self) -> Result<(), DirectInputMouseError> {
        debug!("Handling mouse input lost");
        if let Some(ref mouse_device) = self.mouse_device {
            unsafe {
                match mouse_device.Acquire() {
                    Ok(_) => {
                        debug!("Successfully re-acquired mouse after input lost");
                        Ok(())
                    }
                    Err(e) => {
                        debug!("Failed to re-acquire mouse: {}", e);
                        Err(DirectInputMouseError::InputLost)
                    }
                }
            }
        } else {
            Err(DirectInputMouseError::DeviceNotAcquired)
        }
    }

    #[cfg(windows)]
    fn handle_not_acquired(&mut self) -> Result<(), DirectInputMouseError> {
        debug!("Handling mouse not acquired");
        if let Some(ref mouse_device) = self.mouse_device {
            unsafe {
                match mouse_device.Acquire() {
                    Ok(_) => {
                        debug!("Successfully acquired mouse");
                        Ok(())
                    }
                    Err(e) => {
                        debug!("Failed to acquire mouse: {}", e);
                        Err(DirectInputMouseError::DeviceNotAcquired)
                    }
                }
            }
        } else {
            Err(DirectInputMouseError::DeviceNotAcquired)
        }
    }

    #[cfg(windows)]
    fn update_cursor_position(&mut self) {
        unsafe {
            let mut point = POINT::default();
            if GetCursorPos(&mut point).is_ok() {
                if ScreenToClient(self.app_window, &mut point).is_ok() {
                    self.current_position = Point { x: point.x, y: point.y };
                }
            }
        }
    }
}

impl Drop for DirectInputMouse {
    fn drop(&mut self) {
        self.close();
    }
}

// Thread safety - DirectInput mouse is designed to be used from the main thread only
unsafe impl Send for DirectInputMouse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_io_default() {
        let io = MouseIO::default();
        assert_eq!(io.left_state, MouseButtonState::Up);
        assert_eq!(io.middle_state, MouseButtonState::Up);
        assert_eq!(io.right_state, MouseButtonState::Up);
        assert_eq!(io.pos.x, 0);
        assert_eq!(io.pos.y, 0);
        assert_eq!(io.wheel_pos, 0);
    }

    #[test]
    fn test_mouse_button_state() {
        assert_eq!(MouseButtonState::Up as u8, 0);
        assert_eq!(MouseButtonState::Down as u8, 1);
        assert_eq!(MouseButtonState::DoubleClick as u8, 2);
    }

    #[test]
    fn test_mouse_constants() {
        assert_eq!(MOUSE_NONE, 0x00);
        assert_eq!(MOUSE_OK, 0x01);
        assert_eq!(MOUSE_FAILED, 0x80);
        assert_eq!(MOUSE_LOST, 0xFF);
    }

    #[test]
    fn test_point() {
        let mut point = Point::default();
        assert_eq!(point.x, 0);
        assert_eq!(point.y, 0);
        
        point.x = 100;
        point.y = 200;
        assert_eq!(point.x, 100);
        assert_eq!(point.y, 200);
    }

    #[test]
    fn test_mouse_creation() {
        #[cfg(windows)]
        {
            let mouse = DirectInputMouse::new(HINSTANCE(0), HWND(0));
            assert!(!mouse.is_initialized());
            assert_eq!(mouse.get_error_count(), 0);
            let (buttons, axes, ff) = mouse.get_capabilities();
            assert_eq!(buttons, 3);
            assert_eq!(axes, 3);
            assert!(!ff);
        }
        #[cfg(not(windows))]
        {
            let mouse = DirectInputMouse::new(0 as _, 0 as _);
            assert!(!mouse.is_initialized());
            assert_eq!(mouse.get_error_count(), 0);
        }
    }
}